// Draws one ship on the shared stage: puts ships on it (flying in, pinned
// over a thumbnail, or sliding in beside the last one), runs the frame loop
// and takes the blueprint's pictures. The engine is set up in backend.ts,
// the camera is camera.ts, the section views sections.ts.
import { blue } from '@carbonenginejs/runtime/blue';

import { createBackend, type Backend } from './backend';
import {
  BLUEPRINT_VIEWS,
  assembleBlueprint,
  blueprintDistance,
  hullBounds,
  type Blueprint,
} from './blueprint';
import { BLUEPRINT_FOV, OrbitCamera, pinFrustum, type Rig } from './camera';
import { approach } from './easing';
import { onDrag } from './input';
import { placeSun } from './lighting';
import { placement } from './placement';
import {
  FIELD_OF_VIEW,
  THUMBNAIL_FRAMING,
  easeInOut,
  type CameraPose,
  type ScreenRect,
} from './poses';
import type { GameFiles } from './resources';
import { SectionViews, type Section, type SectionView } from './sections';
import { buildShip } from './ship';

/* eslint-disable @typescript-eslint/no-explicit-any */

/** How quickly the hologram fades in and out, per second. */
const HOLOGRAM_EASING = 3;
/** How quickly the missing-resource glitch comes and goes, per second. */
const GLITCH_EASING = 6;
/** A ship's radius until its model says, in metres. */
const DEFAULT_RADIUS = 50;
/** Livery carousel: the two ships never sit closer than this many combined radii. */
const SLIDE_SPACING = 1.2;
/**
 * How far past the edge of the frame a ship off to the side sits, in its
 * radii: the incoming one while it loads, the outgoing one once the glide
 * ends. Bounding spheres are rough for long hulls (a bow turned toward the
 * edge reaches well past them), so this leaves generous room.
 */
const EDGE_MARGIN = 1.4;
/** How long the camera takes to glide onto the incoming ship, in ms. */
const SLIDE_DURATION = 1300;
/** Frames the resource manager must stay idle before a ship counts as loaded. */
const QUIET_FRAMES = 8;
/** Go on anyway after this long, in ms, rather than wait on a resource that never lands. */
const READY_TIMEOUT = 12000;

const nextFrame = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));

/**
 * Shows only the ship's hull until the returned function puts the rest back.
 * The blueprint draws the hull's outline. A livery can add effects around it
 * (an aurora, a particle cloud) that would widen the drawing, and with it
 * the sheet, so the page would jump on a livery change. Effect children are
 * set aside by swapping in an empty list (the same list goes back, so
 * nothing is reloaded), attachments (sprites, haze, banners, spotlight
 * cones) only draw while the ship is activated, and boosters have a switch.
 */
function hullOnly(ship: any) {
  const { effectChildren, activationStrength } = ship;
  const boosters = ship.boosters;
  const boostersShown = boosters?.display;
  ship.effectChildren = [];
  ship.activationStrength = 0;
  if (boosters) boosters.display = false;
  return () => {
    ship.effectChildren = effectChildren;
    ship.activationStrength = activationStrength;
    if (boosters) boosters.display = boostersShown;
  };
}

/** A livery waiting to slide in. */
interface QueuedSlide {
  values: any;
  direction: 1 | -1;
  onGlide?: () => void;
  resolve: (shown: boolean) => void;
}

/**
 * A slide under way. The ships move, not the camera, and along the screen's
 * right of whichever view draws them (`placeSlide`), so each view sees the
 * new ship come in from the side it should, however it is turned.
 */
interface Slide {
  /** `loading`: the incoming ship waits out of frame; `gliding`: it comes in. */
  phase: 'loading' | 'gliding';
  /** The incoming ship's position: beside the target while it loads, then gliding in. */
  incoming: number[];
  /** Where the incoming ship lands: its visual centre on the target. */
  landing: number[];
  /** The outgoing ship's position, gliding out the other way. */
  outgoing: number[];
  spacing: number;
  /** How much further the outgoing ship travels, so it leaves the frame. */
  push: number;
  direction: 1 | -1;
  fromDistance: number;
  toDistance: number;
  /** When the glide started. */
  start: number;
  /** How far the glide has got, eased, 0 to 1. */
  eased: number;
  resolve: () => void;
}

export class ShipRenderer {
  /** Owned by the renderer, so it can move between pages without a new device. */
  readonly canvas: HTMLCanvasElement;
  ship: any = null;
  /** Called for every resource read that fails, with its res:/ path. */
  onMissing: ((path: string, error: Error) => void) | null = null;

  private backend: Backend;
  private camera = new OrbitCamera();
  private rig: Rig;
  private sections: SectionViews;
  /** 0 is the normal render, 1 the fitting window hologram. */
  private hologramAmount = 0;
  private hologramTarget = 0;
  /** 0 is a clean picture, 1 the full missing-resource glitch. */
  private glitchAmount = 0;
  private glitchTarget = 0;
  /** The resting radius of the ship on stage, for spacing the carousel. */
  private shipRadius = DEFAULT_RADIUS;
  /** Bumped for every ship put on stage: a different ship cancels what waited on the last. */
  private shipToken = 0;
  /**
   * The livery waiting to slide in next. Only one waits: a newer request
   * replaces it, so skipped liveries never play.
   */
  private nextSlide: QueuedSlide | null = null;
  /** Whether the queue is working through slides. */
  private sliding = false;
  private slide: Slide | null = null;
  private slideDone: Promise<void> = Promise.resolve();
  /**
   * Whether the canvas takes the wheel and every drag itself: in a preview
   * fixed to the screen (a docked panel), where there is no page to scroll
   * under it. Otherwise the page scrolls: the wheel zooms only with ctrl, and
   * a finger's vertical swipe scrolls the page while sideways it orbits.
   */
  private ownsScroll = false;
  private lastFrame = 0;
  private reported = new Set<string>();

  static async create(files: GameFiles): Promise<ShipRenderer> {
    const canvas = document.createElement('canvas');
    let renderer: ShipRenderer | null = null;
    const backend = await createBackend(canvas, files, (path, error) =>
      renderer?.onMissing?.(path, error),
    );
    renderer = new ShipRenderer(canvas, backend);
    return renderer;
  }

  private constructor(canvas: HTMLCanvasElement, backend: Backend) {
    this.canvas = canvas;
    this.backend = backend;
    this.rig = {
      view: backend.view,
      projection: backend.projection,
      aspect: () => canvas.width / Math.max(1, canvas.height),
      sun: (yaw, pitch) => placeSun(backend.scene, yaw, pitch),
    };
    this.sections = new SectionViews(backend.gpu);
    this.bindInput();
    requestAnimationFrame((time) => this.frame(time));
  }

  // --- Putting a ship on stage ---

  /**
   * Swaps the shown ship for one built from SOF model values. Resolves once
   * it has loaded and its fly-in has begun, or false when another ship took
   * its place first.
   */
  async setShip(values: any): Promise<boolean> {
    const token = this.replaceShip(values);
    // Drawn from afar meanwhile, so everything it needs is requested.
    this.camera.flyIn();
    const loaded = await this.loaded(token);
    if (token !== this.shipToken) return false;
    this.camera.land();
    return loaded;
  }

  /** Puts a new ship on stage in place of everything shown; returns its request token. */
  private replaceShip(values: any) {
    const token = ++this.shipToken;
    this.nextSlide?.resolve(false);
    this.nextSlide = null;
    // A different ship cancels any livery glide in flight.
    if (this.slide) {
      const { resolve } = this.slide;
      this.slide = null;
      resolve();
    }
    this.removeObject(this.ship);
    this.ship = buildShip(values);
    this.ship.translationCurve = placement([0, 0, 0]);
    this.backend.scene.objects.push(this.ship);
    this.shipRadius = values.boundingSphereRadius ?? DEFAULT_RADIUS;
    this.camera.frame(values.boundingSphereCenter ?? [0, 0, 0], this.shipRadius);
    return token;
  }

  /**
   * Puts a new ship on stage exactly over its thumbnail, at `rect` on screen:
   * the same camera as scripts/render-thumbnails.ts, and the same crop to
   * the hull, measured here from a first look. Resolves once it has loaded
   * and is pinned, or false when another ship took its place first. A ship
   * that drew nothing to measure is shown the normal way instead.
   */
  async pinShip(values: any, rect: ScreenRect): Promise<boolean> {
    const token = this.replaceShip(values);
    const { margin, padding, solid } = THUMBNAIL_FRAMING;
    this.camera.pin((this.shipRadius * margin) / Math.sin(FIELD_OF_VIEW / 2));
    this.hologramAmount = 0;
    // Drawn meanwhile (the stage keeps it out of sight), so everything it
    // needs is requested.
    if (!(await this.loaded(token))) return false;

    // The first look, and the hull's box in it.
    this.camera.apply(this.rig);
    if (!this.pump()) return false;
    const image = await this.backend.hologram.readback();
    if (token !== this.shipToken) return false;
    const bounds = hullBounds(image, solid - 1);
    const screen = this.canvas.getBoundingClientRect();
    if (!bounds || !screen.width) {
      this.camera.unpin();
      return true;
    }
    this.camera.pinTo(pinFrustum(bounds, image, rect, screen, padding));
    this.camera.apply(this.rig);
    this.pump();
    return true;
  }

  /**
   * Lets a pinned ship go: over `duration` ms it moves from its thumbnail's
   * place to the normal framing, then spins as usual.
   */
  releasePin(duration: number) {
    this.camera.release(duration);
  }

  private removeObject(object: any) {
    if (!object) return;
    const { objects } = this.backend.scene;
    const at = objects.indexOf(object);
    if (at !== -1) objects.splice(at, 1);
  }

  /**
   * Waits until the resource manager has stayed idle for a few frames (or
   * gives up waiting after READY_TIMEOUT). With a `token`, false as soon as a
   * different ship, or a newer livery, makes the wait moot.
   */
  private async loaded(token?: number): Promise<boolean> {
    const overtaken = () =>
      token !== undefined && (token !== this.shipToken || this.nextSlide !== null);
    const deadline = performance.now() + READY_TIMEOUT;
    let quiet = 0;
    while (performance.now() < deadline && quiet < QUIET_FRAMES) {
      // Polling once per frame is the point here, not a missed parallelism.
      // oxlint-disable-next-line no-await-in-loop
      await nextFrame();
      if (overtaken()) return false;
      quiet = blue.resMan.IsLoading() ? 0 : quiet + 1;
    }
    return !overtaken();
  }

  // --- The livery carousel ---

  /**
   * The new ship is built beside the current one, out of frame on the side
   * it comes from; once everything it needs has loaded, the camera glides
   * over to it, the old ship leaves on the other side and is dropped.
   * `onGlide` is called as it starts moving. Resolves once it has landed, or
   * false when a newer request overtook this one.
   */
  async slideToShip(values: any, direction: 1 | -1, onGlide?: () => void): Promise<boolean> {
    // A livery that has not started gliding yet gives way to this one.
    this.nextSlide?.resolve(false);
    const shown = new Promise<boolean>((resolve) => {
      this.nextSlide = { values, direction, onGlide, resolve };
    });
    void this.runSlides();
    return shown;
  }

  /**
   * Plays queued slides one after another. A glide that has started always
   * finishes, so rapid clicks never leave the carousel halfway; whatever was
   * clicked meanwhile plays next (only the latest of it).
   */
  private async runSlides() {
    if (this.sliding) return;
    this.sliding = true;
    try {
      while (this.nextSlide) {
        const { values, direction, onGlide, resolve } = this.nextSlide;
        this.nextSlide = null;
        // One slide at a time is the point of the queue.
        // oxlint-disable-next-line no-await-in-loop
        resolve(await this.slideOnce(values, direction, onGlide));
      }
    } finally {
      this.sliding = false;
    }
  }

  private async slideOnce(values: any, direction: 1 | -1, onGlide?: () => void): Promise<boolean> {
    const token = this.shipToken;
    if (!this.ship) {
      const shown = await this.setShip(values);
      if (shown) onGlide?.();
      return shown;
    }

    const radius = values.boundingSphereRadius ?? DEFAULT_RADIUS;
    const center: number[] = values.boundingSphereCenter ?? [0, 0, 0];
    const fromDistance = this.camera.distance;
    const toDistance = OrbitCamera.restFor(radius);

    // The camera whose framing spaces the ships: a section view while one
    // shows, else the main one. (Which way they glide is up to each view.)
    const watching = this.sections.visible[0]?.view ?? null;
    const zoom = watching ? (watching.pose.zoom ?? 1) / watching.scale : this.camera.framing;
    const shift = watching?.shift ?? 0;

    // Spaced by what the camera can see, not by ship size alone: a big hull
    // is framed from further back, where the view is wider. With the ship
    // off centre, the side it sits nearer to has less room than the other.
    const aspect = this.rig.aspect();
    const reach = (distance: number, side: 1 | -1) =>
      distance * zoom * Math.tan(FIELD_OF_VIEW / 2) * aspect * (1 - 2 * shift * side);
    const spacing = Math.max(
      (this.shipRadius + radius) * SLIDE_SPACING,
      reach(fromDistance, direction) + radius * EDGE_MARGIN,
    );
    // Whatever the gap leaves of the outgoing ship inside the final frame, it
    // travels outward during the glide, so it has left the picture before it
    // is removed.
    const push = Math.max(
      0,
      reach(toDistance, direction === 1 ? -1 : 1) + this.shipRadius * EDGE_MARGIN - spacing,
    );

    // The incoming ship's origin is placed so its visual centre lands on the
    // target: its bounding-sphere centre sits off its origin (usually along
    // its length), and otherwise the spin would push one side's ship further
    // into view. It waits beside the target, out of frame, while it loads.
    const landing = this.camera.target.map((value, axis) => value - center[axis]);
    const incoming = buildShip(values);
    incoming.translationCurve = placement([...landing]);
    this.backend.scene.objects.push(incoming);
    this.slideDone = new Promise<void>((resolve) => {
      this.slide = {
        phase: 'loading',
        incoming: incoming.translationCurve.position,
        landing,
        outgoing: this.ship.translationCurve?.position ?? [0, 0, 0],
        spacing,
        push,
        direction,
        fromDistance,
        toDistance,
        start: 0,
        eased: 0,
        resolve,
      };
    });

    if (!(await this.loaded(token))) {
      if (token === this.shipToken) this.slide = null;
      this.removeObject(incoming);
      return false;
    }

    // Glide: the incoming ship moves onto the target and the frame re-fits
    // it, while the outgoing ship drifts out the other way.
    const slide: Slide | null = this.slide;
    if (slide) {
      slide.phase = 'gliding';
      slide.start = performance.now();
    }
    onGlide?.();
    await this.slideDone;
    if (token !== this.shipToken) {
      // Cut short by a new ship; that ship replaced everything already.
      this.removeObject(incoming);
      return false;
    }

    // Swap roles and recenter on the origin, moving ship and camera target
    // together so nothing on screen jumps.
    this.removeObject(this.ship);
    this.ship = incoming;
    this.shipRadius = radius;
    incoming.translationCurve = placement([0, 0, 0]);
    this.camera.target = [...center];
    return true;
  }

  /**
   * Places the gliding ships for a view looking from `yaw`: the incoming one
   * comes in from that view's screen side, the outgoing one leaves the other
   * way, whatever angle the view is dragged to.
   */
  private placeSlide(yaw: number) {
    const slide = this.slide;
    if (!slide) return;
    const right = [Math.cos(yaw), 0, -Math.sin(yaw)];
    const { eased, direction, spacing, push, landing, incoming, outgoing } = slide;
    for (let axis = 0; axis < 3; axis++) {
      incoming[axis] = landing[axis] + right[axis] * spacing * direction * (1 - eased);
      outgoing[axis] = -right[axis] * (spacing + push) * direction * eased;
    }
  }

  /** One frame of a glide: the ships' progress, and the camera's distance with it. */
  private advanceSlide(now: number) {
    const slide = this.slide;
    if (slide?.phase !== 'gliding') return;
    const progress = Math.min(1, (now - slide.start) / SLIDE_DURATION);
    slide.eased = easeInOut(progress);
    // The zoom follows the glide exactly, so the frame is final when it ends.
    const { fromDistance, toDistance } = slide;
    this.camera.rest(fromDistance + (toDistance - fromDistance) * slide.eased);
    if (progress < 1) return;
    // Land exactly; the swap that follows keeps this picture.
    this.placeSlide(this.camera.yaw);
    // The outgoing ship goes now, before this frame draws: with the slide
    // over, the section views no longer place it for their own angle, and
    // where it was left for this one can be inside theirs.
    this.removeObject(this.ship);
    this.slide = null;
    slide.resolve();
  }

  /**
   * While the incoming ship loads, draws it once in view, on its landing
   * spot, in a pass the frame's real renders then paint over. The runtime
   * only requests a ship's textures and translates its shader permutations
   * (synchronously) when it first draws it, and out of frame it is never
   * drawn: without this, a look the scene has not drawn before (a SKINR
   * pattern) would load and compile mid-glide, stalling the glide and
   * popping in. This way the wait for resources covers it, while nothing
   * on screen moves.
   */
  private warmIncoming() {
    const slide = this.slide;
    if (slide?.phase !== 'loading') return;
    for (let axis = 0; axis < 3; axis++) slide.incoming[axis] = slide.landing[axis];
    this.camera.apply(this.rig);
    blue.os.PumpOS();
    this.placeSlide(this.camera.yaw);
  }

  // --- What the page asks of the camera and the picture ---

  /** Back to the starting angle and idle spin. */
  resetView() {
    this.camera.reset();
  }

  /**
   * Moves the camera onto a pose over TRANSITION_MS, then holds it exactly
   * (a pose without a yaw resumes the idle spin). It holds until the viewer drags.
   */
  setPose(pose: CameraPose) {
    this.camera.setPose(pose);
  }

  /**
   * Morphs the camera into the blueprint's side elevation, framed so the
   * canvas shows `halfHeight` metres above and below the ship's centre, or
   * back to the normal camera with null.
   */
  setBlueprint(halfHeight: number | null) {
    this.camera.setBlueprint(halfHeight);
  }

  /**
   * Moves the camera back by `factor` times its framing distance (1: as
   * framed), easing there, or at once with `instant` (a canvas nobody sees).
   */
  setFraming(factor: number, instant = false) {
    this.camera.setFraming(factor, instant);
  }

  /** Fades toward the fitting window hologram (1) or the normal render (0). */
  setHologram(amount: number) {
    this.hologramTarget = Math.min(1, Math.max(0, amount));
  }

  /** Glitches the picture (1) for a ship with missing resources, or clears it (0). */
  setGlitch(amount: number) {
    this.glitchTarget = Math.min(1, Math.max(0, amount));
  }

  /** Whether the canvas takes the wheel and every drag itself (see `ownsScroll`). */
  setOwnsScroll(on: boolean) {
    this.ownsScroll = on;
    this.canvas.style.touchAction = on ? 'none' : 'pan-y';
  }

  /**
   * The canvas of the section view `key`, created on first use. The page puts
   * it in its section; the renderer draws into it while the view is visible.
   */
  sectionCanvas(key: string): HTMLCanvasElement {
    return this.sections.canvas(key);
  }

  /** Shows the section view `key` from its pose while `visible`, or stops it with null. */
  setSectionView(key: string, view: SectionView | null) {
    this.sections.setView(key, view);
  }

  /**
   * Draws the ship's blueprint: side, top and front views in the hologram
   * style, from a near-orthographic camera, each cropped to the hull. Renders
   * them all within one task and then the live view again, so nothing flashes.
   */
  async captureBlueprint(): Promise<Blueprint | null> {
    if (!this.ship) return null;
    await this.loaded();
    if (!this.ship) return null;
    const { hologram } = this.backend;
    const distance = blueprintDistance(this.shipRadius, this.rig.aspect());
    const reads: Promise<ImageData>[] = [];
    const showAll = hullOnly(this.ship);
    try {
      // Its scale comes from `distance` alone, so no page framing may add to it.
      this.camera.look({ fov: BLUEPRINT_FOV, distance, zoom: 1, framing: 1, blueprint: 0 }, () => {
        for (const view of BLUEPRINT_VIEWS) {
          this.camera.yaw = view.yaw;
          this.camera.pitch = view.pitch;
          this.camera.apply(this.rig);
          this.pump();
          hologram.apply(1, 0);
          reads.push(hologram.readback());
        }
      });
    } finally {
      showAll();
      this.camera.apply(this.rig);
      this.pump();
      if (this.hologramAmount > 0) hologram.apply(this.hologramAmount, 0);
    }
    return assembleBlueprint(await Promise.all(reads), distance, this.canvas.height);
  }

  // --- The frame loop ---

  private frame(time: number) {
    requestAnimationFrame((next) => this.frame(next));
    const seconds = this.lastFrame ? (time - this.lastFrame) / 1000 : 0;
    this.lastFrame = time;
    // Nothing to draw while neither canvas is on a page.
    if (!this.canvas.isConnected && !this.sections.connected) return;
    const now = performance.now();
    this.camera.update(seconds, now);
    this.sections.update(seconds);
    this.advanceSlide(now);
    this.hologramAmount = approach(
      this.hologramAmount,
      this.hologramTarget,
      HOLOGRAM_EASING,
      seconds,
    );
    this.glitchAmount = approach(this.glitchAmount, this.glitchTarget, GLITCH_EASING, seconds);
    try {
      this.warmIncoming();
      if (this.ship) {
        for (const { section, view } of this.sections.visible) {
          this.renderSection(section, view, time);
        }
      }
      this.placeSlide(this.camera.yaw);
      this.camera.apply(this.rig);
      blue.os.PumpOS();
      if (this.hologramAmount > 0 || this.glitchAmount > 0) {
        this.backend.hologram.apply(this.hologramAmount, time / 1000, this.glitchAmount);
      }
    } catch (error) {
      this.recoverFrame(error);
    }
  }

  /** Renders a section view into the main canvas and copies it to the section's canvas. */
  private renderSection(section: Section, view: SectionView, time: number) {
    if (!section.context) return;
    const { hologram } = this.backend;
    const { width, height } = this.canvas;
    if (section.canvas.width !== width || section.canvas.height !== height) {
      section.canvas.width = width;
      section.canvas.height = height;
    }
    const angles = this.sections.angles(section, view.pose);
    // A smaller ship is the same ship seen from further back.
    const zoom = (view.pose.zoom ?? 1) / view.scale;
    this.camera.look({ ...angles, zoom, lensShift: view.shift, blueprint: 0 }, () => {
      this.placeSlide(angles.yaw);
      this.camera.apply(this.rig);
      blue.os.PumpOS();
      if (this.glitchAmount > 0) hologram.apply(0, time / 1000, this.glitchAmount);
      hologram.copyTo(section.context, { x: 0, y: 0, width, height });
    });
  }

  /** Renders one frame through the runtime; a frame that fails is recovered from. */
  private pump() {
    try {
      blue.os.PumpOS();
      return true;
    } catch (error) {
      this.recoverFrame(error);
      return false;
    }
  }

  /**
   * A frame that threw midway leaves the WebGPU work queue open, and it then
   * refuses every later frame: close it so the next one starts clean. One
   * failed frame must not stop rendering; each distinct failure is reported once.
   */
  private recoverFrame(error: unknown) {
    const { al } = this.backend;
    // oxlint-disable-next-line no-underscore-dangle -- the queue exposes no getter for it
    if (al.GetWorkQueue?.()?._inFrame) {
      try {
        al.EndScene();
      } catch {
        // Nothing more to salvage from this frame.
      }
    }
    const message = String(error);
    if (!this.reported.has(message)) {
      this.reported.add(message);
      console.error(error);
    }
  }

  private bindInput() {
    const { canvas, camera } = this;
    this.setOwnsScroll(this.ownsScroll);
    onDrag(canvas, {
      start: () => {
        camera.dragging = true;
      },
      // The camera is the viewer's only once they drag: a finger that lands
      // on the ship to scroll the page must not stop its spin.
      move: (dx, dy) => camera.orbit(dx, dy),
      end: () => {
        camera.dragging = false;
      },
    });
    canvas.addEventListener(
      'wheel',
      (event) => {
        // Where the page scrolls under the canvas, only zoom on purpose
        // (ctrl/pinch or while dragging); in a fixed preview, always.
        if (!event.ctrlKey && !camera.dragging && !this.ownsScroll) return;
        event.preventDefault();
        camera.wheel(event.deltaY, this.shipRadius);
      },
      { passive: false },
    );
  }
}

// The one 3D stage every page shares. It lives in the root layout above the
// page content; pages say where it should be with a scene (`useScene`: the
// full-screen hero, the preview panel, the blueprint) and what to show, and
// it moves between those places on its own (stage-machine.ts).
import type { Place } from './stage-machine';
import type { ShipSummary } from './types';
import { HERO_POSE, type CameraPose, type ScreenRect } from './viewer/poses';
import type { Blueprint, EngineStage } from './viewer/engine';

export interface StagePlacement {
  /** Horizontal shift, in viewport widths. */
  x: number;
  /** Vertical shift, in viewport heights (a phone's ship above its text). */
  y?: number;
  scale: number;
  /**
   * How much farther back the camera sits (1: as framed). Scaling the canvas
   * cannot fit a ship wider than the canvas; pulling the camera back can.
   */
  framing?: number;
}

export const CENTER: StagePlacement = { x: 0, scale: 1 };

/**
 * What a page wants of the stage while it is mounted. Only `place` is needed;
 * the rest default to a centred ship, the hero pose, no dimming and the
 * normal render.
 */
export interface Scene {
  place: Place;
  /** Where the hero sits beside the page's text. */
  placement?: StagePlacement;
  /** The camera angle. */
  pose?: CameraPose;
  /** 0 to 1: dims the hero at the end of a page. */
  dim?: number;
  /** 0 to 1: the fitting window hologram over the ship. */
  hologram?: number;
  /**
   * In the blueprint: how far the hull reaches from its centre along its
   * length, in metres. The stage then covers the slot and frames the camera
   * so the slot's width is exactly the hull's length, at the drawing's scale.
   */
  fit?: { halfLength: number } | null;
  /**
   * Whether a different ship slides in from a side like a livery does,
   * instead of flying in (the explorer's panel, the SKINR pages).
   */
  slideShips?: boolean;
  /** Whether the page shows a blueprint; the stage draws one once the ship is ready. */
  draft?: boolean;
}

/** A leap to another ship (#lib/leap.svelte.ts) and how far along it is. */
export interface Leap {
  /**
   * `load`: the ship builds pinned over its card's picture at `rect`, out of
   * sight; `pop`: it shows and pops out to the middle as the page fades;
   * `land`: the cover lifts off the new page.
   */
  phase: 'load' | 'pop' | 'land';
  rect: ScreenRect;
}

/** How many lines of the stage's trace to keep. */
const TRACE_LINES = 50;

export type StageStatus = 'idle' | 'starting' | 'loading' | 'ready' | 'error';

export interface DockRect {
  /** Position in the viewport. */
  x: number;
  y: number;
  /**
   * Position in the page (viewport plus scroll). Unlike x and y it does not
   * change while scrolling, so something placed with it scrolls with the page.
   */
  pageX: number;
  pageY: number;
  width: number;
  height: number;
  /**
   * Whether the element is fixed to the viewport (it or an ancestor is
   * position: fixed). The stage then stays fixed too instead of scrolling
   * with the page.
   */
  fixed: boolean;
}

/** Whether an element or any ancestor is fixed to the viewport. */
function isFixed(node: HTMLElement) {
  for (let element: HTMLElement | null = node; element; element = element.parentElement) {
    if (getComputedStyle(element).position === 'fixed') return true;
  }
  return false;
}

class Stage {
  /** What the page on screen wants; null while no page has a stage. */
  scene = $state.raw<Scene | null>(null);
  /** The ship on stage, and the DNA it is built from (its livery). */
  ship = $state<ShipSummary | null>(null);
  dna = $state<string | null>(null);
  /** Every place on the page the stage can dock into, measured by `dockTarget`. */
  docks = $state<Partial<Record<Place, DockRect | null>>>({});
  /** Where the page wants the stage; null while no page has a stage. */
  get place(): Place | null {
    return this.scene?.place ?? null;
  }
  /** The viewport rectangle of the dock the page wants, once measured. */
  get dock(): DockRect | null {
    const place = this.place;
    return place && place !== 'hero' ? (this.docks[place] ?? null) : null;
  }
  get placement(): StagePlacement {
    return this.scene?.placement ?? CENTER;
  }
  get pose(): CameraPose {
    return this.scene?.pose ?? HERO_POSE;
  }
  get dim(): number {
    return this.scene?.dim ?? 0;
  }
  get hologram(): number {
    return this.scene?.hologram ?? 0;
  }
  get blueprintFit(): { halfLength: number } | null {
    return this.scene?.fit ?? null;
  }
  get slideShips(): boolean {
    return this.scene?.slideShips ?? false;
  }
  get wantBlueprint(): boolean {
    return this.scene?.draft ?? false;
  }
  /** The drawing of the ship on stage, and the DNA it was drawn from. */
  blueprint = $state<{ dna: string; drawing: Blueprint } | null>(null);
  status = $state<StageStatus>('idle');
  error = $state<string | null>(null);
  /** How far the engine has started, for the boot screen. */
  engineStage = $state<EngineStage | null>(null);
  /**
   * Whether the visit's first ship is loaded and on its way in (or there is
   * none to wait for, or it failed). Until then the boot screen covers the
   * page and the page holds its animations (boot-screen.svelte).
   */
  booted = $state(false);
  /**
   * The res:/ paths the ship on stage could not load (not in the client's
   * file index, or refused by the CDN). Cleared for every new ship or livery.
   */
  missing = $state<string[]>([]);
  /** Whether the ship on stage is broken: it failed to build, or is missing files. */
  get broken() {
    return this.status === 'error' || this.missing.length > 0;
  }
  /**
   * The ship whose name morphs between pages (a card's label and the hero
   * title). Only one element per page may carry a view-transition-name, so
   * cards name themselves only when they hold this ship.
   */
  morphId = $state<number | null>(null);
  /**
   * The ship whose hull is on screen: it changes as a new ship starts
   * flying or sliding in, not when it is asked for, so a page can change
   * its labels in step with the hull.
   */
  onScreen = $state<ShipSummary | null>(null);
  /** The DNA of the hull on screen (`onScreen`'s look), in step with it. */
  onScreenDna = $state<string | null>(null);
  /** Which side the next livery of the same ship slides in from. */
  direction: 1 | -1 = 1;
  /**
   * A leap to another ship under way: the stage covers the viewport, whatever
   * the page wants, until it ends.
   */
  leap = $state.raw<Leap | null>(null);
  /** The stage's last state changes and navigations, oldest first (`trace`). */
  history: string[] = [];
  /** Whether the trace also goes to the console (the debug tools, or dev). */
  debug = false;
  /** Set by the stage component once the renderer exists. */
  resetView: (() => void) | null = null;
  /** Lets the pinned ship of a leap move into the normal framing over that many ms. */
  releasePin: ((duration: number) => void) | null = null;

  /**
   * A SOF pattern the DNA needs that the SOF does not ship (a SKINR design's),
   * registered with the engine before the build.
   */
  pattern: { name: string; record: object } | null = null;

  /** Notes a state change or a navigation in the stage's history. */
  trace(line: string) {
    const stamped = `${Math.round(performance.now())} ${line}`;
    this.history.push(stamped);
    if (this.history.length > TRACE_LINES) this.history.shift();
    if (this.debug) console.debug(`[stage] ${stamped}`);
  }

  show(
    ship: ShipSummary,
    dna = ship.dna,
    direction: 1 | -1 = this.direction,
    pattern: { name: string; record: object } | null = null,
  ) {
    this.direction = direction;
    this.pattern = pattern;
    this.ship = ship;
    this.dna = dna;
  }
}

export const stage = new Stage();

/** Keeps a viewport rectangle current while an element lays out, resizes or scrolls. */
function trackRect(node: HTMLElement, set: (rect: DockRect | null) => void) {
  const fixed = isFixed(node);
  const measure = () => {
    const rect = node.getBoundingClientRect();
    set(
      rect.width > 0 && rect.height > 0
        ? {
            x: rect.x,
            y: rect.y,
            pageX: rect.x + window.scrollX,
            pageY: rect.y + window.scrollY,
            width: rect.width,
            height: rect.height,
            fixed,
          }
        : null,
    );
  };
  measure();
  const observer = new ResizeObserver(measure);
  observer.observe(node);
  window.addEventListener('resize', measure);
  // A fixed element does not move when the page scrolls.
  if (!fixed) window.addEventListener('scroll', measure, { passive: true });
  return {
    destroy() {
      observer.disconnect();
      window.removeEventListener('resize', measure);
      if (!fixed) window.removeEventListener('scroll', measure);
      set(null);
    },
  };
}

/** The page holding the stage now (`useScene`). */
let owner: object | null = null;

/**
 * Puts the stage where a component wants it for as long as the component is
 * mounted. Call while it initialises; `null` says it shows no stage now (My
 * skins logged out), and the stage fades out at once. A page that goes hands
 * the stage straight to the next one, whatever order they mount and unmount
 * in; only when no next page takes it (one without a stage) does it go, a
 * frame later.
 */
export function useScene(scene: () => Scene | null) {
  const token = {};
  $effect(() => {
    const next = scene();
    owner = token;
    stage.scene = next;
  });
  $effect(() => () => {
    requestAnimationFrame(() => {
      if (owner !== token) return;
      owner = null;
      stage.scene = null;
    });
  });
}

/** Marks an element as one of the places the stage can dock into. */
export function dockTarget(node: HTMLElement, name: Exclude<Place, 'hero'>) {
  return trackRect(node, (rect) => {
    stage.docks[name] = rect;
  });
}

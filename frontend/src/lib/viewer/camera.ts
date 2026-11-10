// The camera that orbits the ship on stage. What it is doing is explicit
// state, three small machines side by side:
//
// - its motion: spinning idly, holding still, or on a timed move to a pose;
// - its lens: normal, pinned over a thumbnail (a leap), or being released;
// - a new ship's arrival: waiting far out while it loads, then landing.
//
// It knows nothing of the engine: `apply` writes the view and projection it
// is handed, so the logic here runs in a test.
import { approach } from './easing';
import {
  FIELD_OF_VIEW,
  FRAMING_RADII,
  HERO_POSE,
  REST_PITCH,
  REST_YAW,
  THUMBNAIL_POSE,
  TRANSITION_MS,
  easeInOut,
  type CameraPose,
  type ScreenRect,
} from './poses';

/** Radians per dragged pixel. */
const ORBIT_SPEED = 0.008;
/** How far up or down a drag may tilt the camera, in radians. */
const MAX_PITCH = 1.5;
/** Radians per second the camera drifts until the viewer touches it. */
export const IDLE_SPIN = 0.1;
/** A new ship flies in from this many times its resting distance… */
const FLY_IN_FACTOR = 2.4;
/** …and from this much higher, in radians of pitch. */
const FLY_IN_PITCH = 0.35;
/** How quickly the camera eases toward its resting distance, per second. */
const DISTANCE_EASING = 2.2;
/** How quickly a ship flying in eases down to its pose's pitch, per second. */
const PITCH_EASING = 1.2;
/** How quickly the camera follows a new framing factor (a dock resized or entered), per second. */
const FRAMING_EASING = 6;
/** How much one wheel step (a pixel of deltaY) changes the framing distance, exponentially. */
const WHEEL_ZOOM = 0.002;
/** How close and how far the wheel may take the camera, in normal framing distances. */
const ZOOM_IN_LIMIT = 0.4;
const ZOOM_OUT_LIMIT = 3;
/** The camera never rests closer to a ship's centre than this, in metres. */
const MIN_DISTANCE = 10;
/** Where the camera rests before any ship has said its size, in metres. */
const DEFAULT_DISTANCE = 150;
/** The near plane sits at this share of the camera's distance, but no closer than MIN_NEAR. */
const NEAR_SHARE = 1 / 100;
const MIN_NEAR = 0.1;
/** The far plane, in camera distances. */
const FAR_FACTOR = 20;
/**
 * Blueprint views use a narrow field of view from far away: close enough to
 * orthographic that the three views share one scale, as a drawing should.
 */
export const BLUEPRINT_FOV = (6 * Math.PI) / 180;

/**
 * A view frustum in tangent units (the near plane's edges divided by its
 * distance), so it holds at any near plane.
 */
export interface Frustum {
  left: number;
  right: number;
  bottom: number;
  top: number;
}

/** What a timed move blends: the orbit's angles, the pose's zoom, the blueprint framing. */
interface Angles {
  yaw: number;
  pitch: number;
  zoom: number;
  blueprint: number;
}

/** What turns the camera. */
type Motion =
  /** Drifting round the ship until the viewer touches it. */
  | { kind: 'spin' }
  /** Still: a pose with a yaw, or wherever the viewer dragged it. */
  | { kind: 'hold' }
  /** On its way to `to`, landing exactly on it; then spinning or holding. */
  | { kind: 'tween'; from: Angles; to: Angles; start: number; spinAfter: boolean };

/** What the lens shows. */
type Lens =
  | { kind: 'normal' }
  /**
   * Over a thumbnail: the thumbnail's camera, its lens moved so the hull
   * lands exactly where the picture is on screen (`frustum`, once measured).
   */
  | { kind: 'pinned'; distance: number; frustum: Frustum | null }
  /** Easing from the pin into the normal framing. */
  | { kind: 'releasing'; distance: number; frustum: Frustum; start: number; duration: number };

/** A new ship coming in. */
type Arrival =
  | { kind: 'none' }
  /**
   * Held far out while the ship loads: flying in while textures stream and
   * shaders compile stutters and pops. `pitch` is where it will ease down to.
   */
  | { kind: 'waiting'; pitch: number | null }
  /** Loaded: easing down to `pitch` as the distance closes. */
  | { kind: 'landing'; pitch: number };

/** What `apply` writes to: the engine's view and projection, and the sun that rides with the camera. */
export interface Rig {
  view: { SetLookAtPosition(eye: number[], target: number[], up: number[]): void };
  projection: {
    PerspectiveFov(fov: number, aspect: number, near: number, far: number): void;
    PerspectiveOffCenter(
      left: number,
      right: number,
      bottom: number,
      top: number,
      near: number,
      far: number,
    ): void;
  };
  /** The canvas's width over its height. */
  aspect(): number;
  sun(yaw: number, pitch: number): void;
}

/** What a temporary look may change (`OrbitCamera.look`). */
export interface Look {
  yaw: number;
  pitch: number;
  distance: number;
  zoom: number;
  framing: number;
  blueprint: number;
  fov: number;
  lensShift: number;
}

/** Orbit angles after a drag of `dx`, `dy` pixels. */
export function orbited(angles: { yaw: number; pitch: number }, dx: number, dy: number) {
  return {
    yaw: angles.yaw - dx * ORBIT_SPEED,
    pitch: Math.min(MAX_PITCH, Math.max(-MAX_PITCH, angles.pitch + dy * ORBIT_SPEED)),
  };
}

/** The shortest way round from `from` to the angle `to`. */
function shortestTurn(from: number, to: number) {
  const turn = Math.PI * 2;
  return from + ((((to - from) % turn) + turn * 1.5) % turn) - turn / 2;
}

export class OrbitCamera {
  yaw = REST_YAW;
  pitch = REST_PITCH;
  /** Where the camera sits now, and where it is easing to. */
  distance = DEFAULT_DISTANCE;
  restDistance = DEFAULT_DISTANCE;
  /** The pose's distance factor. */
  zoom = 1;
  /**
   * How much farther the camera sits than the ship's framing asks: in a dock
   * narrower than the canvas, so the ship fits the dock (stage.svelte), eased.
   */
  framing = 1;
  private framingTarget = 1;
  /**
   * The blueprint framing: 0 is the normal camera, 1 the near-orthographic
   * side elevation showing `blueprintHalfHeight` metres above and below the
   * ship's centre. Moved by a tween, so the camera morphs between them.
   */
  blueprint = 0;
  private blueprintTarget = 0;
  private blueprintHalfHeight = 0;
  /** The field of view; blueprint captures narrow it for a moment. */
  fov = FIELD_OF_VIEW;
  /** How far right of centre the lens is shifted, as a share of the canvas width. */
  lensShift = 0;
  /** The point the camera looks at: the ship's visual centre. */
  target = [0, 0, 0];
  /** Whether the viewer has a pointer down on the canvas. */
  dragging = false;
  /** The pose the page asked for last; a new ship flies in onto it. */
  private pose: CameraPose = HERO_POSE;
  private motion: Motion = { kind: 'spin' };
  private lens: Lens = { kind: 'normal' };
  private arrival: Arrival = { kind: 'none' };

  /** The states, for a debugger or a test. */
  get state() {
    return { motion: this.motion.kind, lens: this.lens.kind, arrival: this.arrival.kind };
  }

  /** The camera's rest distance for a ship of `radius`. */
  static restFor(radius: number) {
    return Math.max(MIN_DISTANCE, radius * FRAMING_RADII);
  }

  /** Looks at a new ship: its centre and size. Any pin goes with the old one. */
  frame(center: number[], radius: number) {
    this.target = [...center];
    this.restDistance = OrbitCamera.restFor(radius);
    this.lens = { kind: 'normal' };
  }

  /** Sits exactly `distance` away and rests there (a livery glide sets it each frame). */
  rest(distance: number) {
    this.distance = this.restDistance = distance;
  }

  /**
   * Starts a new ship's arrival: from afar and a little higher, onto the pose
   * the page asked for, where it waits until `land`. A viewer mid-drag keeps
   * their view.
   */
  flyIn() {
    if (this.dragging) {
      this.arrival = { kind: 'waiting', pitch: null };
      return;
    }
    this.distance = this.restDistance * FLY_IN_FACTOR;
    this.pitch = this.pose.pitch + FLY_IN_PITCH;
    this.arrival = { kind: 'waiting', pitch: this.pose.pitch };
  }

  /** The ship has loaded: the arrival comes in. */
  land() {
    if (this.arrival.kind !== 'waiting') return;
    const { pitch } = this.arrival;
    this.arrival = pitch === null ? { kind: 'none' } : { kind: 'landing', pitch };
  }

  /** The viewer or a pose takes the pitch over from an arrival. */
  private keepPitch() {
    if (this.arrival.kind === 'waiting') this.arrival = { kind: 'waiting', pitch: null };
    else if (this.arrival.kind === 'landing') this.arrival = { kind: 'none' };
  }

  /**
   * Holds the thumbnail's angle from `distance`, with nothing else on the
   * camera, until `pinTo` says where the hull must land.
   */
  pin(distance: number) {
    this.lens = { kind: 'pinned', distance, frustum: null };
    this.motion = { kind: 'hold' };
    this.arrival = { kind: 'none' };
    ({ yaw: this.yaw, pitch: this.pitch } = THUMBNAIL_POSE);
    this.zoom = 1;
    this.blueprint = this.blueprintTarget = 0;
    this.distance = this.restDistance;
  }

  /** The pinned lens, once the hull has been measured. */
  pinTo(frustum: Frustum) {
    if (this.lens.kind === 'pinned') this.lens = { ...this.lens, frustum };
  }

  /** Drops a pin that could not be measured: the ship shows the normal way. */
  unpin() {
    this.lens = { kind: 'normal' };
  }

  /**
   * Lets a pinned ship go: over `duration` ms it moves from its thumbnail's
   * place to the normal framing, then spins as usual.
   */
  release(duration: number, now = performance.now()) {
    if (this.lens.kind !== 'pinned') return;
    const { distance, frustum } = this.lens;
    this.lens = frustum
      ? { kind: 'releasing', distance, frustum, start: now, duration }
      : { kind: 'normal' };
  }

  /**
   * Moves the camera onto a pose over TRANSITION_MS, then holds it exactly
   * (a pose without a yaw resumes the idle spin). It holds until the viewer drags.
   */
  setPose(pose: CameraPose, now = performance.now()) {
    this.pose = pose;
    this.keepPitch();
    const yaw = pose.yaw === undefined ? this.yaw : shortestTurn(this.yaw, pose.yaw);
    this.tweenTo(
      { yaw, pitch: pose.pitch, zoom: pose.zoom ?? 1, blueprint: this.blueprintTarget },
      pose.yaw === undefined,
      now,
    );
  }

  /** Back to the starting angle and idle spin. */
  reset(now = performance.now()) {
    this.setPose(HERO_POSE, now);
    this.distance = Math.max(this.distance, this.restDistance);
  }

  /**
   * Morphs the camera into the blueprint's side elevation, framed so the
   * canvas shows `halfHeight` metres above and below the ship's centre, or
   * back to the normal camera with null.
   */
  setBlueprint(halfHeight: number | null, now = performance.now()) {
    const target = halfHeight === null ? 0 : 1;
    if (halfHeight !== null) this.blueprintHalfHeight = halfHeight;
    this.blueprintTarget = target;
    if (this.motion.kind === 'tween') this.motion.to.blueprint = target;
    else if (this.blueprint !== target) {
      this.tweenTo({ ...this.angles(), blueprint: target }, this.motion.kind === 'spin', now);
    }
  }

  /**
   * Moves the camera back by `factor` times its framing distance (1: as
   * framed), easing there, or at once with `instant` (a canvas nobody sees).
   */
  setFraming(factor: number, instant = false) {
    this.framingTarget = Math.max(1, factor);
    if (instant) this.framing = this.framingTarget;
  }

  /**
   * The viewer drags by `dx`, `dy` pixels: the camera is theirs from here,
   * and stays where they leave it.
   */
  orbit(dx: number, dy: number) {
    this.motion = { kind: 'hold' };
    this.keepPitch();
    ({ yaw: this.yaw, pitch: this.pitch } = orbited(this, dx, dy));
  }

  /** The wheel moves the resting distance, within limits, for a ship of `radius`. */
  wheel(deltaY: number, radius: number) {
    const framed = OrbitCamera.restFor(radius);
    this.restDistance = Math.min(
      framed * ZOOM_OUT_LIMIT,
      Math.max(framed * ZOOM_IN_LIMIT, this.restDistance * Math.exp(deltaY * WHEEL_ZOOM)),
    );
  }

  private angles(): Angles {
    return { yaw: this.yaw, pitch: this.pitch, zoom: this.zoom, blueprint: this.blueprint };
  }

  private tweenTo(to: Angles, spinAfter: boolean, now: number) {
    this.motion = { kind: 'tween', from: this.angles(), to, start: now, spinAfter };
  }

  /** One frame of every eased value: `seconds` since the last, at time `now`. */
  update(seconds: number, now: number) {
    const lens = this.lens;
    if (lens.kind === 'releasing' && now >= lens.start + lens.duration) {
      // Landed on the normal framing: from here on it is the usual camera.
      this.lens = { kind: 'normal' };
      this.distance = this.restDistance;
      if (this.motion.kind !== 'tween') {
        this.motion = { kind: this.pose.yaw === undefined ? 'spin' : 'hold' };
      }
    }
    if (this.motion.kind === 'spin') this.yaw += seconds * IDLE_SPIN;
    if (this.arrival.kind !== 'waiting') {
      this.distance = approach(this.distance, this.restDistance, DISTANCE_EASING, seconds);
    }
    const motion = this.motion;
    if (motion.kind === 'tween') {
      const t = Math.min(1, (now - motion.start) / TRANSITION_MS);
      const k = easeInOut(t);
      const { from, to } = motion;
      this.yaw = from.yaw + (to.yaw - from.yaw) * k;
      this.pitch = from.pitch + (to.pitch - from.pitch) * k;
      this.zoom = from.zoom + (to.zoom - from.zoom) * k;
      this.blueprint = from.blueprint + (to.blueprint - from.blueprint) * k;
      if (t >= 1) {
        ({ yaw: this.yaw, pitch: this.pitch, zoom: this.zoom, blueprint: this.blueprint } = to);
        this.motion = { kind: motion.spinAfter ? 'spin' : 'hold' };
      }
    } else if (this.arrival.kind === 'landing') {
      // The fly-in of a new ship eases its pitch on its own.
      this.pitch = approach(this.pitch, this.arrival.pitch, PITCH_EASING, seconds);
      if (this.pitch === this.arrival.pitch) this.arrival = { kind: 'none' };
    }
    this.framing = approach(this.framing, this.framingTarget, FRAMING_EASING, seconds);
  }

  /**
   * Draws something from another look (a section's view, a blueprint
   * elevation) and puts the camera back as it was, whatever `draw` changed.
   */
  look<T>(look: Partial<Look>, draw: () => T): T {
    const saved: Look = {
      yaw: this.yaw,
      pitch: this.pitch,
      distance: this.distance,
      zoom: this.zoom,
      framing: this.framing,
      blueprint: this.blueprint,
      fov: this.fov,
      lensShift: this.lensShift,
    };
    try {
      Object.assign(this, look);
      return draw();
    } finally {
      Object.assign(this, saved);
    }
  }

  /** The orbit's eye point and up direction at `distance` (its own up, so straight down is a valid view too). */
  private orbitFrom(distance: number) {
    const [tx, ty, tz] = this.target;
    const c = Math.cos(this.pitch);
    return {
      eye: [
        tx + distance * c * Math.sin(this.yaw),
        ty + distance * Math.sin(this.pitch),
        tz + distance * c * Math.cos(this.yaw),
      ],
      up: [
        -Math.sin(this.pitch) * Math.sin(this.yaw),
        Math.cos(this.pitch),
        -Math.sin(this.pitch) * Math.cos(this.yaw),
      ],
    };
  }

  /** Writes the camera to the engine's view and projection. */
  apply(rig: Rig, now = performance.now()) {
    if (this.lens.kind !== 'normal') {
      this.applyPinned(rig, this.lens, now);
      return;
    }
    // Blend the framing by visible half-height rather than by distance, so the
    // ship keeps its size on screen while the field of view narrows.
    const blend = this.blueprint;
    const fov = this.fov + (BLUEPRINT_FOV - this.fov) * blend;
    const normalHalfHeight = this.distance * this.zoom * this.framing * Math.tan(this.fov / 2);
    const halfHeight = normalHalfHeight + (this.blueprintHalfHeight - normalHalfHeight) * blend;
    const distance = halfHeight / Math.tan(fov / 2);
    const { eye, up } = this.orbitFrom(distance);
    rig.view.SetLookAtPosition(eye, this.target, up);
    rig.sun(this.yaw, this.pitch);
    const aspect = rig.aspect();
    const near = Math.max(MIN_NEAR, distance * NEAR_SHARE);
    const far = distance * FAR_FACTOR;
    if (this.lensShift) {
      // The same frustum, slid sideways: what the camera looks at lands
      // `lensShift` of the width right of centre.
      const top = near * Math.tan(fov / 2);
      const half = top * aspect;
      const center = -2 * this.lensShift * half;
      rig.projection.PerspectiveOffCenter(center - half, center + half, -top, top, near, far);
    } else {
      rig.projection.PerspectiveFov(fov, aspect, near, far);
    }
  }

  /** The camera for a pinned ship, blended toward the normal one while it is released. */
  private applyPinned(rig: Rig, lens: Exclude<Lens, { kind: 'normal' }>, now: number) {
    const tangent = Math.tan(FIELD_OF_VIEW / 2);
    const aspect = rig.aspect();
    const normal: Frustum = {
      left: -tangent * aspect,
      right: tangent * aspect,
      bottom: -tangent,
      top: tangent,
    };
    const from = lens.frustum ?? normal;
    const k =
      lens.kind === 'releasing' ? easeInOut(Math.min(1, (now - lens.start) / lens.duration)) : 0;
    const mix = (a: number, b: number) => a + (b - a) * k;
    const distance = mix(lens.distance, this.restDistance);
    const { eye, up } = this.orbitFrom(distance);
    rig.view.SetLookAtPosition(eye, this.target, up);
    rig.sun(this.yaw, this.pitch);
    const near = Math.max(MIN_NEAR, distance * NEAR_SHARE);
    rig.projection.PerspectiveOffCenter(
      mix(from.left, normal.left) * near,
      mix(from.right, normal.right) * near,
      mix(from.bottom, normal.bottom) * near,
      mix(from.top, normal.top) * near,
      near,
      distance * FAR_FACTOR,
    );
  }
}

/**
 * The lens that puts a hull exactly over its thumbnail. `bounds` is the
 * hull's box in `image`, a first look from the thumbnail's camera; the
 * thumbnail shows that box made square and padded by `padding`. The frustum
 * is stretched over the whole canvas (at `screen`) so that square covers
 * `rect`. All in tangent units.
 */
export function pinFrustum(
  bounds: { left: number; right: number; top: number; bottom: number },
  image: { width: number; height: number },
  rect: ScreenRect,
  screen: { x: number; y: number; width: number; height: number },
  padding: number,
): Frustum {
  const tangent = Math.tan(FIELD_OF_VIEW / 2);
  const aspect = image.width / image.height;
  const x = (pixel: number) => ((pixel / image.width) * 2 - 1) * tangent * aspect;
  const y = (pixel: number) => (1 - (pixel / image.height) * 2) * tangent;
  const centerX = (x(bounds.left) + x(bounds.right + 1)) / 2;
  const centerY = (y(bounds.top) + y(bounds.bottom + 1)) / 2;
  const extent =
    (Math.max(x(bounds.right + 1) - x(bounds.left), y(bounds.top) - y(bounds.bottom + 1)) / 2) *
    padding;
  const perPixel = (extent * 2) / rect.width;
  const left = centerX - extent - (rect.x - screen.x) * perPixel;
  const top = centerY + extent + (rect.y - screen.y) * perPixel;
  return {
    left,
    right: left + screen.width * perPixel,
    top,
    bottom: top - screen.height * perPixel,
  };
}

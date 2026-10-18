// Camera poses: the angles pages frame a ship from. Kept apart from the
// renderer so page state can name them without loading the engine.

/** The resting angle: slightly above the ship, three-quarter view. */
export const REST_YAW = 0.6;
/** The resting pitch: slightly above the ship. */
export const REST_PITCH = 0.22;
/** The camera's vertical field of view, in radians. */
export const FIELD_OF_VIEW = Math.PI / 4;

/**
 * A camera pose. One with a `yaw` holds that angle; one without spins slowly.
 * `zoom` scales the framing distance.
 */
export interface CameraPose {
  yaw?: number;
  pitch: number;
  zoom?: number;
}

/**
 * The thumbnails' angle (scripts/render-thumbnails.ts): the hero view's
 * resting three-quarter angle, held, so every ship is shown the same way.
 */
export const THUMBNAIL_POSE = { yaw: REST_YAW, pitch: REST_PITCH };

/**
 * How a thumbnail is framed. A first look takes the bounding sphere with
 * `margin` room (spheres are loose, so this only has to keep the hull in
 * frame); then the view closes in on the hull's own pixels (those at least
 * `solid` opaque), made square and padded by `padding`. The renderer frames
 * a ship the same way to pin it over its thumbnail.
 */
export const THUMBNAIL_FRAMING = { margin: 1.25, padding: 1.06, solid: 8 };

/** A rectangle on screen, in CSS pixels from the viewport's top left. */
export interface ScreenRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** The three-quarter hero view, slowly spinning. */
export const HERO_POSE: CameraPose = { pitch: REST_PITCH };

/** Broadside, level: the ship's profile. */
export const SIDE_POSE: CameraPose = { yaw: Math.PI / 2, pitch: 0.02, zoom: 0.95 };

/** Straight down onto the hull (just short of vertical, which the look-at cannot take). */
export const TOP_POSE: CameraPose = { yaw: 0, pitch: 1.45, zoom: 0.95 };

/** Low and from the front quarter, for showing off a livery. */
export const FRONT_POSE: CameraPose = { yaw: -0.75, pitch: 0.12, zoom: 0.9 };

/** The blueprint's side elevation: level and exactly broadside (renderer.setBlueprint frames it). */
export const BLUEPRINT_POSE: CameraPose = { yaw: Math.PI / 2, pitch: 0, zoom: 1 };

/** The fitting window's slow turn: from above, spinning. */
export const FITTING_POSE: CameraPose = { pitch: 0.5, zoom: 0.92 };

/** The other flank, level. */
export const OTHER_SIDE_POSE: CameraPose = { yaw: -Math.PI / 2, pitch: 0.08, zoom: 0.95 };

/**
 * How long a change of camera pose or framing takes, in ms. Matches the
 * stage's move between places so both land together; afterwards the camera
 * holds the new pose exactly.
 */
export const TRANSITION_MS = 900;

/** Ease-in-out cubic: the curve every place and pose transition follows. */
export const easeInOut = (t: number) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);

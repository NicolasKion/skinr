// The ship page on a narrow screen (a phone): the hero and profile ship sits
// in the upper part of the screen, above the text, which scrolls up over it.
import type { StagePlacement } from '#lib/stage.svelte.js';
import { SPHERE_SPAN } from '#lib/viewer/poses.js';

/** The ship's centre sits this far above the screen's middle, in screen heights… */
const NARROW_Y = -0.22;
/** …framed so its sphere fills this much of the width. */
const NARROW_FILL = 0.9;
/**
 * How much of the framed sphere's lower half the text keeps clear of: a
 * hull seldom reaches the sphere's bottom, so the text may start inside it.
 */
const NARROW_CLEAR = 0.75;
/** How far the text rises over the ship while it fades, in screen heights. */
const NARROW_FADE = 0.25;

interface Size {
  width: number;
  height: number;
}

/** How much of its framed size the ship keeps, so it fits the width. */
export function narrowFit({ width, height }: Size): number {
  return Math.min(1, (width * NARROW_FILL) / (height * SPHERE_SPAN));
}

/** The stage's placement: above the text, the camera back far enough to fit. */
export function narrowPlacement(size: Size): StagePlacement {
  return { x: 0, y: NARROW_Y, scale: 1, framing: 1 / narrowFit(size) };
}

/** Where the ship's framed sphere starts and where the text may start under it, in px from the top. */
function shipBand(size: Size) {
  const sphere = size.height * SPHERE_SPAN * narrowFit(size);
  const centre = size.height * (0.5 + NARROW_Y);
  return { top: centre - sphere / 2, bottom: centre + (sphere / 2) * NARROW_CLEAR };
}

/**
 * Where the hero starts its text, below the ship: the band's bottom, in CSS
 * so the server renders it right too (the same sum as `shipBand`).
 */
export const NARROW_CLEAR_CSS = `calc(100svh * ${0.5 + NARROW_Y} + min(100svh * ${SPHERE_SPAN}, ${NARROW_FILL * 100}vw) * ${NARROW_CLEAR / 2})`;

/**
 * How far the ship has faded (0 to 1) under text scrolling up over it:
 * `texts` are the text blocks' boxes in the viewport.
 */
export function textCover(size: Size, texts: ({ top: number; bottom: number } | null)[]): number {
  const ship = shipBand(size);
  const fade = size.height * NARROW_FADE;
  return Math.max(
    0,
    ...texts.map((text) =>
      text ? Math.min(1, (ship.bottom - text.top) / fade, (text.bottom - ship.top) / fade) : 0,
    ),
  );
}

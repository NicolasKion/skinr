// The ship's blueprint: side, top and front elevations captured from a
// near-orthographic camera, each cropped to the hull. The renderer takes the
// pictures (`captureBlueprint`); this is how they are framed, cropped and
// measured.
import { BLUEPRINT_FOV } from './camera';
import { encodePng } from './png';

/** Room around the ship in a blueprint view, in bounding-sphere radii. */
const BLUEPRINT_MARGIN = 1.08;
/** Pixels of padding kept around the ship when a view is cropped. */
const BLUEPRINT_PADDING = 6;
/** A captured pixel counts as hull above this alpha (0 to 255). */
const HULL_ALPHA = 8;
/**
 * The drawn views. The side elevation is the live ship itself (see
 * setBlueprint), seen from the same yaw as the plan, so length runs left to
 * right in both.
 */
export const BLUEPRINT_VIEWS = [
  // Also drawn statically, so the sheet keeps its ship once the live one leaves.
  { name: 'side', yaw: Math.PI / 2, pitch: 0 },
  { name: 'top', yaw: Math.PI / 2, pitch: Math.PI / 2 },
  { name: 'front', yaw: 0, pitch: 0 },
] as const;

export interface BlueprintView {
  name: (typeof BLUEPRINT_VIEWS)[number]['name'];
  url: string;
  /** Cropped size in canvas pixels; every view shares one scale. */
  width: number;
  height: number;
  /** Where the ship's centre falls in the cropped image, in its pixels. */
  centerX: number;
  centerY: number;
}

export interface Blueprint {
  views: BlueprintView[];
  /** The drawings' scale. */
  metersPerPixel: number;
  /** Bounding-box size in metres, from the plan (length, beam) and front (height). */
  length: number;
  beam: number;
  height: number;
  /**
   * How far the hull reaches from its centre, in metres, along its length and
   * up or down: the box the live side elevation has to fit, centred on the ship.
   */
  halfLength: number;
  halfHeight: number;
}

/** The box around a captured frame's pixels more opaque than `solid`, or null when there are none. */
export function hullBounds(image: ImageData, solid: number) {
  const { width, height, data } = image;
  let left = width;
  let right = -1;
  let top = height;
  let bottom = -1;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (data[(y * width + x) * 4 + 3] > solid) {
        if (x < left) left = x;
        if (x > right) right = x;
        if (y < top) top = y;
        if (y > bottom) bottom = y;
      }
    }
  }
  return right < 0 ? null : { left, right, top, bottom };
}

/** Crops a captured view to the hull's visible pixels; null when nothing was drawn. */
async function crop(image: ImageData) {
  const { width, height, data } = image;
  const bounds = hullBounds(image, HULL_ALPHA);
  if (!bounds) return null;
  let { left, top } = bounds;
  const { right, bottom } = bounds;
  left = Math.max(0, left - BLUEPRINT_PADDING);
  top = Math.max(0, top - BLUEPRINT_PADDING);
  const cropWidth = Math.min(width, right + BLUEPRINT_PADDING + 1) - left;
  const cropHeight = Math.min(height, bottom + BLUEPRINT_PADDING + 1) - top;
  const png = await encodePng(data, width, left, top, cropWidth, cropHeight);
  return {
    url: URL.createObjectURL(png),
    width: cropWidth,
    height: cropHeight,
    // The camera looks at the ship's centre, which is the frame's centre.
    centerX: width / 2 - left,
    centerY: height / 2 - top,
  };
}

/**
 * How far the blueprint's camera sits from a ship of `radius`. Framed by the
 * canvas's height; a portrait canvas (a phone) is narrower than tall, so the
 * camera backs off until the hull fits its width too, or the drawing would
 * measure a hull cut off at the sides.
 */
export function blueprintDistance(radius: number, aspect: number) {
  return (radius / Math.tan(BLUEPRINT_FOV / 2)) * BLUEPRINT_MARGIN * Math.max(1, 1 / aspect);
}

/**
 * The blueprint from one captured frame per view (in BLUEPRINT_VIEWS order),
 * taken from `distance` on a canvas `canvasHeight` pixels tall.
 */
export async function assembleBlueprint(
  images: ImageData[],
  distance: number,
  canvasHeight: number,
): Promise<Blueprint> {
  const cropped = await Promise.all(images.map((image) => crop(image)));
  const views: BlueprintView[] = [];
  BLUEPRINT_VIEWS.forEach((view, index) => {
    const found = cropped[index];
    if (found) views.push({ name: view.name, ...found });
  });
  const metersPerPixel = (2 * distance * Math.tan(BLUEPRINT_FOV / 2)) / canvasHeight;
  const top = views.find((view) => view.name === 'top');
  const front = views.find((view) => view.name === 'front');
  return {
    views,
    metersPerPixel,
    length: (top?.width ?? 0) * metersPerPixel,
    beam: (top?.height ?? 0) * metersPerPixel,
    height: (front?.height ?? 0) * metersPerPixel,
    halfLength: top ? Math.max(top.centerX, top.width - top.centerX) * metersPerPixel : 0,
    halfHeight: front ? Math.max(front.centerY, front.height - front.centerY) * metersPerPixel : 0,
  };
}

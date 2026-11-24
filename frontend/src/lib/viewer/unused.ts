// Game files a ship or its scene names that this viewer never draws. Leaving
// them out is most of a first visit: the backdrop alone is 15 MB, the hit
// effects another 9 MB, and a ship is shown only once nothing is loading.

/**
 * The scene's backdrop is never drawn (the canvas is transparent), so its
 * 24 MB nebula cube is read as the scene's small blurred one and the
 * starfield as a black texture.
 */
const STAND_INS: Record<string, string> = {
  'res:/dx9/scene/universe/c01_cube.dds': 'res:/dx9/scene/universe/c01_cube_blur.dds',
  'res:/dx9/scene/starfield/stars01_tile2.dds': 'res:/texture/global/black.dds',
};

/**
 * What only a ship's impact overlay reads: shield and armour hits, which
 * nothing here triggers (`withoutHitEffects` drops the overlay).
 */
const HIT_EFFECT_FILES =
  /\/fisfx\/impact\/|\/texture\/fx\/3d\/3dfx_shield|shieldimpactgradient|unitsphere_shieldgeo|\/fx\/impact\//;

/**
 * Post-processing is not driven here, yet its renderer asks for the film
 * grain's 770 kB noise as soon as it exists. It gets one transparent pixel.
 */
const NEVER_READ = new Set(['res:/texture/global/film_grain_noise.png']);

/** A 1x1 transparent PNG. */
const BLANK_PNG = Uint8Array.from(
  atob(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  ),
  (character) => character.charCodeAt(0),
);

/** The bytes to hand out for a file that is never read from the game, else null. */
export function blank(path: string): Uint8Array | null {
  return NEVER_READ.has(path.toLowerCase()) ? BLANK_PNG.slice() : null;
}

/** The path to read for `path`: itself, or the small file standing in for it. */
export function standIn(path: string): string {
  return STAND_INS[path.toLowerCase()] ?? path;
}

/** A ship's SOF values without its shield and armour hit effects. */
export function withoutHitEffects<T extends object>(values: T): T {
  return { ...values, impactOverlay: null };
}

/**
 * The paths worth fetching ahead out of those a render recorded: lists drawn
 * before these cuts still name the backdrop, the hit effects and the grain.
 */
export function worthPreloading(paths: string[]): string[] {
  const wanted = new Set<string>();
  for (const path of paths) {
    const name = path.toLowerCase();
    if (!HIT_EFFECT_FILES.test(name) && !NEVER_READ.has(name)) wanted.add(standIn(path));
  }
  return [...wanted];
}

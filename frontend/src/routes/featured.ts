import type { ShipSummary } from '#lib/types.js';

/** Ships worth opening on, one picked at random for a first visit. */
const FEATURED = new Set([
  'Raven',
  'Megathron',
  'Apocalypse',
  'Tempest',
  'Astero',
  'Nightmare',
  'Vindicator',
]);

/**
 * The ship the explorer opens on: a featured one by `chance` (0 up to 1), or
 * the catalog's first when none of them is in it.
 */
export function pickFeatured(ships: ShipSummary[], chance: number): ShipSummary | undefined {
  const candidates = ships.filter((ship) => FEATURED.has(ship.name));
  return candidates[Math.floor(chance * candidates.length)] ?? ships[0];
}

// The ships a ship page offers next, at its end.
import type { ShipDetail, ShipSummary } from '#lib/types.js';

/** The most cards a rail lists. */
const RAIL_LIMIT = 24;

export interface RelatedRail {
  title: string;
  note: string;
  ships: ShipSummary[];
  /** Whether a card names its ship's class (not where the rail is one class). */
  showClass: boolean;
}

/** A hull's model family: the first part of its SOF hull name (gc4 for gc4_t2a). */
const hullFamily = (dna: string) => dna.split(':')[0].split('_')[0];

/** A ship's page data as the catalog lists ships. */
export function summaryOf(ship: ShipDetail): ShipSummary {
  return {
    id: ship.id,
    name: ship.name,
    group_id: ship.group.id,
    race_id: ship.race?.id ?? null,
    faction_id: ship.faction?.id ?? null,
    meta_group_id: ship.meta_group?.id ?? null,
    tech_level: ship.tech_level,
    dna: ship.dna,
    colors: ship.colors,
  };
}

/**
 * Related ships, in rails from closest to loosest: the same frame, the same
 * class, the same builder. A ship appears only in the first rail it fits,
 * and empty rails are left out.
 */
export function relatedRails(ship: ShipDetail, ships: ShipSummary[]): RelatedRail[] {
  const listed = new Set([ship.id]);
  const byTechThenName = (a: ShipSummary, b: ShipSummary) =>
    (a.tech_level ?? 0) - (b.tech_level ?? 0) || a.name.localeCompare(b.name);
  const take = (fits: (other: ShipSummary) => boolean) => {
    const found = ships
      .filter((other) => !listed.has(other.id) && fits(other))
      .sort(byTechThenName)
      .slice(0, RAIL_LIMIT);
    for (const other of found) listed.add(other.id);
    return found;
  };
  const family = hullFamily(ship.dna);
  const builder = ship.faction
    ? {
        title: `More from ${ship.faction.name}`,
        fits: (other: ShipSummary) => other.faction_id === ship.faction?.id,
      }
    : {
        title: `More ${ship.race?.name ?? 'unaffiliated'} ships`,
        fits: (other: ShipSummary) => other.race_id === (ship.race?.id ?? null),
      };
  return [
    {
      title: 'Same frame',
      note: `Built on the ${ship.name}'s hull, fitted for other roles`,
      ships: take((other) => hullFamily(other.dna) === family),
      showClass: true,
    },
    {
      title: `More ${ship.group.name.toLowerCase()}s`,
      note: 'The same class from other builders',
      ships: take((other) => other.group_id === ship.group.id),
      showClass: false,
    },
    {
      title: builder.title,
      note: 'Every class they field',
      ships: take(builder.fits),
      showClass: true,
    },
  ].filter((rail) => rail.ships.length);
}

// EVE image server URLs, race tints, builder names and number formatting.
import type { Catalog, ShipSummary } from './types';

/** CCP's public image server. */
const IMAGE_SERVER = 'https://images.evetech.net';

export type IconSize = 32 | 64 | 128 | 256 | 512;

export function typeIcon(typeId: number, size: IconSize = 64): string {
  return `${IMAGE_SERVER}/types/${typeId}/icon?size=${size}`;
}

/** The studio render of a ship type, the art the market window shows. */
export function typeRender(typeId: number, size: IconSize = 512): string {
  return `${IMAGE_SERVER}/types/${typeId}/render?size=${size}`;
}

/** The accent each race's pages are tinted with, by SDE race id. */
const RACE_TINTS: Record<number, string> = {
  1: '#7fb2e5', // Caldari: steel blue
  2: '#e0875a', // Minmatar: rust
  4: '#e8c170', // Amarr: gold
  8: '#7fd1a8', // Gallente: green
  16: '#b7a6ff', // Jove: violet
  32: '#ef6b6b', // Pirate: red
  128: '#f2c14e', // ORE: yellow
  135: '#e2564b', // Triglavian: crimson
  168: '#9fb3c8', // Upwell: slate
};

/** A neutral silver for ships without a race. */
export const DEFAULT_TINT = '#a1b5c9';

export function raceTint(raceId: number | null | undefined): string {
  return (raceId != null && RACE_TINTS[raceId]) || DEFAULT_TINT;
}

const number = new Intl.NumberFormat('en-US', { maximumFractionDigits: 1 });

/** A figure with its unit, or a dash when the SDE has none. */
export function formatUnit(value: number | null, unit: string): string {
  return value === null ? '-' : `${number.format(value)} ${unit}`;
}

/** A large figure in the compact form spec sheets use: 99.3M, 470K. */
export function formatCompact(value: number | null): string {
  if (value === null) return '-';
  return new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 }).format(
    value,
  );
}

/** The first sentence of a description, for a tagline. */
export function firstSentence(text: string): string {
  const match = text.match(/^.*?[.!?](\s|$)/s);
  return (match ? match[0] : text).trim();
}

/** Who builds a ship, as cards, labels and the explorer's tabs name it. */
export interface Builder {
  /** `race:<id>` or `faction:<id>`. */
  key: string;
  name: string;
  /** Its emblem: the faction's, or for a race its empire's. */
  logo: string | null;
}

/**
 * Sorts ships by who builds them. CCP's data gives every hull the race whose
 * technology it is built on, so the Sisters of EVE's Astero is Gallente and
 * the Guristas' Gila Caldari. Ships of a race's own empire (the SDE's
 * `empire_of`, see src/sde/parse.rs) belong to the race; every other ship
 * with a faction belongs to that faction. `all` lists the builders with
 * ships: the races in catalog order, then the factions by name.
 */
export function shipBuilders(catalog: Catalog): {
  of: (ship: ShipSummary) => Builder;
  all: Builder[];
} {
  const races = new Map(catalog.races.map((race) => [race.id, race.name]));
  const factions = new Map(catalog.factions.map((faction) => [faction.id, faction]));
  const empires = new Map(
    catalog.factions.flatMap((faction) =>
      faction.empire_of === null ? [] : [[faction.empire_of, faction] as const],
    ),
  );
  const UNAFFILIATED: Builder = { key: 'none', name: 'Unaffiliated', logo: null };
  const of = (ship: ShipSummary): Builder => {
    const faction = ship.faction_id === null ? undefined : factions.get(ship.faction_id);
    if (faction && (faction.empire_of === null || faction.empire_of !== ship.race_id)) {
      return { key: `faction:${faction.id}`, name: faction.name, logo: faction.logo_url };
    }
    const race = ship.race_id === null ? undefined : races.get(ship.race_id);
    if (race) {
      const empire = empires.get(ship.race_id ?? -1);
      return { key: `race:${ship.race_id}`, name: race, logo: empire?.logo_url ?? null };
    }
    return UNAFFILIATED;
  };
  const used = new Map<string, Builder>();
  for (const ship of catalog.ships) {
    const builder = of(ship);
    if (builder !== UNAFFILIATED) used.set(builder.key, builder);
  }
  const all = [
    ...catalog.races.flatMap((race) => used.get(`race:${race.id}`) ?? []),
    ...[...used.values()]
      .filter((builder) => builder.key.startsWith('faction:'))
      .sort((a, b) => a.name.localeCompare(b.name)),
  ];
  return { of, all };
}

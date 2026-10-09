// EVE image server URLs, race tints and number formatting.

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

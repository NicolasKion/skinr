// The site's two looks. Dark is the default; light helps with dark hulls,
// which disappear against a near-black page. The choice lives in a cookie so
// the server renders the right one and the page never flashes the other.

export type Theme = 'dark' | 'light';

/** The cookie holding the visitor's choice. */
export const THEME_COOKIE = 'theme';
/** How long the choice is remembered: a year, in seconds. */
export const THEME_MAX_AGE = 60 * 60 * 24 * 365;

/** The theme a cookie value names, dark for anything else. */
export function parseTheme(value: string | undefined | null): Theme {
  return value === 'light' ? 'light' : 'dark';
}

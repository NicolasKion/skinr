// The width of the explorer's peeking stage, which the visitor can drag. It
// is kept as a share of the viewport, so it scales with the window, and in a
// cookie (like the theme) so the server renders it and nothing jumps.

/** The cookie holding the visitor's width. */
export const PEEK_COOKIE = 'peek';
/** How long the width is remembered: a year, in seconds. */
export const PEEK_MAX_AGE = 60 * 60 * 24 * 365;

/** The width until the visitor drags it: a generous third of the screen. */
export const PEEK_DEFAULT = 'clamp(24rem, 34vw, 38rem)';
/** The narrowest stage, in rem: room for a ship's name and the Open button. */
export const PEEK_MIN_REM = 22;
/** The widest, as a share of the viewport: the shelves keep a card and their arrows. */
export const PEEK_MAX_SHARE = 0.65;
/** How far one arrow-key press moves the edge, as a share of the viewport. */
export const PEEK_KEY_STEP = 0.02;

/** The share a cookie value holds, or null for the default (missing or not a share). */
export function parsePeek(value: string | undefined | null): number | null {
  const share = Number(value);
  return value && Number.isFinite(share) && share > 0 && share < 1 ? share : null;
}

/** The CSS width for a share, the default for null. The clamp keeps it in bounds at any size. */
export function peekWidth(share: number | null): string {
  if (share === null) return PEEK_DEFAULT;
  return `clamp(${PEEK_MIN_REM}rem, ${+(share * 100).toFixed(2)}vw, ${PEEK_MAX_SHARE * 100}vw)`;
}

/** Writes the width to the cookie, or forgets it for null. */
export function savePeek(share: number | null) {
  document.cookie =
    share === null
      ? `${PEEK_COOKIE}=; path=/; max-age=0; samesite=lax`
      : `${PEEK_COOKIE}=${share.toFixed(4)}; path=/; max-age=${PEEK_MAX_AGE}; samesite=lax`;
}

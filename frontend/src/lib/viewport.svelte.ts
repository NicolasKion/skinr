// The viewport's size, for everything laid out by the screen: the stage's
// canvas, the chapters, where the ship sits. Its height is the small
// viewport's (CSS `100svh`), which holds still while a phone's browser bar
// slides in and out as the page scrolls. `window.innerHeight` and `dvh`
// change with the bar, and anything sized by them is laid out again in the
// middle of a scroll: the canvas resizes, the chapters grow, the ship moves.
// So size by `svh` in CSS and read `viewport` in script; never `dvh` or
// `innerHeight` for layout.
import { browser } from '$app/env';

/** The size the server renders for, before a browser has said its own. */
const SERVER_SIZE = { width: 1440, height: 900 };
/**
 * From here up a screen is wide: Tailwind's `lg`, where the preview panel
 * shows and a chapter's text sits beside its ship. Script asks `viewport.wide`;
 * CSS uses `lg:` or this same query.
 */
const WIDE = '(min-width: 64rem)';

/** An element as tall as the small viewport, to read `100svh` in pixels. */
function probe(): HTMLElement {
  const element = document.createElement('div');
  element.style.cssText =
    'position:fixed;top:0;left:0;width:0;height:100svh;visibility:hidden;pointer-events:none';
  document.body.append(element);
  return element;
}

class Viewport {
  /** The viewport's width in CSS pixels. */
  width = $state(SERVER_SIZE.width);
  /** The small viewport's height in CSS pixels: `100svh`. */
  height = $state(SERVER_SIZE.height);
  /** Whether the screen is wide (`lg` and up); the server renders the wide layout. */
  wide = $state(true);

  constructor() {
    if (!browser) return;
    // Measured at once, so a phone never lays out for the server's size first.
    const small = probe();
    this.width = window.innerWidth;
    this.height = small.offsetHeight || window.innerHeight;
    new ResizeObserver(() => {
      this.height = small.offsetHeight || window.innerHeight;
    }).observe(small);
    window.addEventListener('resize', () => {
      this.width = window.innerWidth;
    });
    const wide = matchMedia(WIDE);
    this.wide = wide.matches;
    wide.addEventListener('change', () => {
      this.wide = wide.matches;
    });
  }
}

export const viewport = new Viewport();

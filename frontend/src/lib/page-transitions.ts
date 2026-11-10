// How the page itself moves from one page to another: one table for every
// pair of areas. +layout.svelte runs the result as a View Transition and
// layout.css has the animations (`data-nav`, `data-peek-static`). The 3D
// stage moves itself meanwhile (stage-machine.ts): pages only say where they
// want it.

/** The site's areas, by route. */
export type Area = 'explorer' | 'ship' | 'hub' | 'designer' | 'me';

const AREAS: Record<string, Area> = {
  '/': 'explorer',
  '/ships/[ship]/[...livery]': 'ship',
  '/skinr': 'hub',
  '/skinr/designer/[designer]': 'designer',
  '/me': 'me',
};

/**
 * The areas in the order the site bar reads them (Ships, a ship, the Paragon
 * Hub, a designer, your own skins): moving right through them the page
 * slides in from the right, moving left from the left.
 */
const ORDER: Area[] = ['explorer', 'ship', 'hub', 'designer', 'me'];

/** The areas with the preview panel; between two of them it stays put. */
const WITH_PANEL = new Set<Area>(['explorer', 'hub', 'designer', 'me']);

/**
 * How the page changes (layout.css):
 * - `sweep-forward`, `sweep-back`: explorer and ship page sit side by side,
 *   and the page shifts over in step with the stage sweeping out of its
 *   panel or back into it.
 * - `area-forward`, `area-back`: a short slide the way the site bar reads.
 * - `morph`: a designer's portrait and name move between the hub's chip and
 *   their page's header while the pages cross-fade around them.
 * - `fade`: a plain cross-fade.
 */
export type PageSlide =
  | 'sweep-forward'
  | 'sweep-back'
  | 'area-forward'
  | 'area-back'
  | 'morph'
  | 'fade';

export interface PagePlan {
  slide: PageSlide;
  /** Both pages have the preview panel, which stays put. */
  peekStatic: boolean;
}

/** One end of a navigation. */
export interface PageStop {
  routeId: string | null;
  pathname: string;
  href: string;
  /** The ship page's ship, from its params. */
  ship?: string;
}

/** The area of a route, if it is one of the site's. */
export function areaOf(routeId: string | null): Area | null {
  return (routeId && AREAS[routeId]) || null;
}

/**
 * How the page moves from `from` to `to`, or null when it does not (the same
 * page: a filter, a livery of the same ship; the stage animates those). A
 * wide screen has the preview panel the explorer's ship sweeps out of.
 */
export function planPage(
  from: PageStop | null,
  to: PageStop | null,
  wide: boolean,
): PagePlan | null {
  if (!from || !to || from.href === to.href || from.pathname === to.pathname) return null;
  const a = areaOf(from.routeId);
  const b = areaOf(to.routeId);
  if (a === 'ship' && b === 'ship' && from.ship === to.ship) return null;
  return {
    slide: slideBetween(a, b, wide),
    peekStatic: a !== null && b !== null && WITH_PANEL.has(a) && WITH_PANEL.has(b),
  };
}

function slideBetween(a: Area | null, b: Area | null, wide: boolean): PageSlide {
  // A phone has no panel to sweep from: its explorer and ship page slide
  // like any other two areas.
  if (wide && a === 'explorer' && b === 'ship') return 'sweep-forward';
  if (wide && a === 'ship' && b === 'explorer') return 'sweep-back';
  if ((a === 'hub' && b === 'designer') || (a === 'designer' && b === 'hub')) return 'morph';
  if (a && b && a !== b) return ORDER.indexOf(b) > ORDER.indexOf(a) ? 'area-forward' : 'area-back';
  return 'fade';
}

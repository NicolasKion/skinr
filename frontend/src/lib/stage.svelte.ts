// The one 3D stage every page shares. It lives in the root layout above the
// page content; pages say where it should be (full-screen hero, a dock
// beside the explorer, or hidden) and what to show, and it animates between
// those places on its own.
import type { ShipSummary } from './types';
import { HERO_POSE, type CameraPose, type ScreenRect } from './viewer/poses';
import type { Blueprint, EngineStage } from './viewer/engine';

export interface StagePlacement {
  /** Horizontal shift, in viewport widths. */
  x: number;
  scale: number;
}

export const CENTER: StagePlacement = { x: 0, scale: 1 };

export type StageMode = 'hidden' | 'hero' | 'dock';
export type StageStatus = 'idle' | 'starting' | 'loading' | 'ready' | 'error';

export interface DockRect {
  /** Position in the viewport. */
  x: number;
  y: number;
  /**
   * Position in the page (viewport plus scroll). Unlike x and y it does not
   * change while scrolling, so something placed with it scrolls with the page.
   */
  pageX: number;
  pageY: number;
  width: number;
  height: number;
  /**
   * Whether the element is fixed to the viewport (it or an ancestor is
   * position: fixed). The stage then stays fixed too instead of scrolling
   * with the page.
   */
  fixed: boolean;
}

/** Whether an element or any ancestor is fixed to the viewport. */
function isFixed(node: HTMLElement) {
  for (let element: HTMLElement | null = node; element; element = element.parentElement) {
    if (getComputedStyle(element).position === 'fixed') return true;
  }
  return false;
}

class Stage {
  mode = $state<StageMode>('hidden');
  /** The ship on stage, and the DNA it is built from (its livery). */
  ship = $state<ShipSummary | null>(null);
  dna = $state<string | null>(null);
  /**
   * Every place on the page the stage can dock into, by name, measured by
   * `dockTarget`; `dockName` picks the one in use.
   */
  docks = $state<Record<string, DockRect | null>>({});
  dockName = $state('dock');
  /** The viewport rectangle the stage docks into now. */
  get dock(): DockRect | null {
    return this.docks[this.dockName] ?? null;
  }
  /**
   * Where the hero stage sits: a horizontal shift in viewport widths and a
   * scale, so a page can move the ship beside its text. Eased by the stage.
   */
  placement = $state<StagePlacement>(CENTER);
  /** The camera angle the page wants; applied by the stage component. */
  pose = $state<CameraPose>(HERO_POSE);
  /** 0 to 1: dims the stage at the end of a page. */
  dim = $state(0);
  /** 0 to 1: the fitting window hologram over the ship. */
  hologram = $state(0);
  /** The dock's corner radius in CSS pixels. */
  dockRadius = $state(32);
  /** Whether the page shows a blueprint; the stage draws one once the ship is ready. */
  wantBlueprint = $state(false);
  /**
   * Set while the stage docks into the blueprint's side elevation: how far
   * the hull reaches from its centre along its length, in metres. The stage
   * then covers the slot and frames the camera so the slot's width is exactly
   * the hull's length, at the drawing's scale.
   */
  blueprintFit = $state<{ halfLength: number } | null>(null);
  /** The drawing of the ship on stage, and the DNA it was drawn from. */
  blueprint = $state<{ dna: string; drawing: Blueprint } | null>(null);
  status = $state<StageStatus>('idle');
  error = $state<string | null>(null);
  /** How far the engine has started, for the boot screen. */
  engineStage = $state<EngineStage | null>(null);
  /**
   * Whether the visit's first ship is loaded and on its way in (or there is
   * none to wait for, or it failed). Until then the boot screen covers the
   * page and the page holds its animations (boot-screen.svelte).
   */
  booted = $state(false);
  /**
   * The res:/ paths the ship on stage could not load (not in the client's
   * file index, or refused by the CDN). Cleared for every new ship or livery.
   */
  missing = $state<string[]>([]);
  /** Whether the ship on stage is broken: it failed to build, or is missing files. */
  get broken() {
    return this.status === 'error' || this.missing.length > 0;
  }
  /**
   * The ship whose name morphs between pages (a card's label and the hero
   * title). Only one element per page may carry a view-transition-name, so
   * cards name themselves only when they hold this ship.
   */
  morphId = $state<number | null>(null);
  /**
   * The ship whose hull is on screen: it changes as a new ship starts
   * flying or sliding in, not when it is asked for, so a page can change
   * its labels in step with the hull.
   */
  onScreen = $state<ShipSummary | null>(null);
  /**
   * Whether a different ship slides in from a side like a livery does,
   * instead of flying in (the explorer's dock, a carousel of hulls).
   */
  slideShips = $state(false);
  /** Which side the next livery of the same ship slides in from. */
  direction: 1 | -1 = 1;
  /**
   * A leap to another ship (#lib/leap.svelte.ts): the stage covers the
   * viewport and builds the next ship pinned over its card's picture at
   * `rect`, out of sight until `shown`.
   */
  leap = $state<{ rect: ScreenRect; shown: boolean } | null>(null);
  /** Set by the stage component once the renderer exists. */
  resetView: (() => void) | null = null;
  /** Lets the pinned ship of a leap move into the normal framing over that many ms. */
  releasePin: ((duration: number) => void) | null = null;

  /**
   * A SOF pattern the DNA needs that the SOF does not ship (a SKINR design's),
   * registered with the engine before the build.
   */
  pattern: { name: string; record: object } | null = null;

  show(
    ship: ShipSummary,
    dna = ship.dna,
    direction: 1 | -1 = this.direction,
    pattern: { name: string; record: object } | null = null,
  ) {
    this.direction = direction;
    this.pattern = pattern;
    this.ship = ship;
    this.dna = dna;
  }
}

export const stage = new Stage();

/** Keeps a viewport rectangle current while an element lays out, resizes or scrolls. */
function trackRect(node: HTMLElement, set: (rect: DockRect | null) => void) {
  const fixed = isFixed(node);
  const measure = () => {
    const rect = node.getBoundingClientRect();
    set(
      rect.width > 0 && rect.height > 0
        ? {
            x: rect.x,
            y: rect.y,
            pageX: rect.x + window.scrollX,
            pageY: rect.y + window.scrollY,
            width: rect.width,
            height: rect.height,
            fixed,
          }
        : null,
    );
  };
  measure();
  const observer = new ResizeObserver(measure);
  observer.observe(node);
  window.addEventListener('resize', measure);
  // A fixed element does not move when the page scrolls.
  if (!fixed) window.addEventListener('scroll', measure, { passive: true });
  return {
    destroy() {
      observer.disconnect();
      window.removeEventListener('resize', measure);
      if (!fixed) window.removeEventListener('scroll', measure);
      set(null);
    },
  };
}

/** Marks an element the stage can dock into, under a name (default 'dock'). */
export function dockTarget(node: HTMLElement, name = 'dock') {
  return trackRect(node, (rect) => {
    stage.docks[name] = rect;
  });
}

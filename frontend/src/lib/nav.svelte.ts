import type { Snippet } from 'svelte';

// What the site bar shows besides the logo. Pages fill it in while they are
// mounted (a ship page: its name, its sections, a reset button) and clear it
// when they go, so the bar in the root layout stays page-agnostic.

export interface NavSection {
  /** The id of the element the link scrolls to. */
  id: string;
  label: string;
}

class Nav {
  /** The page's own crumb after "Ships", such as the ship's name. */
  title = $state<string | null>(null);
  /** In-page sections, linked on the right of the bar. */
  sections = $state<NavSection[]>([]);
  /** The section in view, highlighted in the bar. */
  active = $state<string | null>(null);
  /** Puts the page's view back where it started (the 3D camera). */
  reset = $state<(() => void) | null>(null);
  /**
   * Controls the page docks into the middle of the bar (the explorer's
   * filters), and whether they show there now; hidden, they stay mounted so
   * they can slide in without a layout jump.
   */
  center = $state<Snippet | null>(null);
  centerShown = $state(false);

  clear() {
    this.title = null;
    this.sections = [];
    this.active = null;
    this.reset = null;
    this.center = null;
    this.centerShown = false;
  }
}

export const nav = new Nav();

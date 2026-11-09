// The width the visitor dragged the preview panel to this visit, shared by
// every page that has one (#lib/components/peek-panel.svelte); until a drag,
// the width the server rendered from the cookie (#lib/peek.ts) holds.
import { page } from '$app/state';
import { peekWidth } from './peek.js';

class PeekState {
  /** A share of the viewport, null for the default, undefined: not dragged yet. */
  dragged = $state<number | null | undefined>(undefined);
  /** Whether the panel's edge is being dragged right now. */
  resizing = $state(false);

  /**
   * The panel's width as CSS, for `--peek`. The property does not inherit
   * (layout.css), so it goes on each element that uses it, the panel's layers
   * and the page's main column: set on an ancestor, every change during a
   * drag would restyle the whole page.
   */
  get css(): string {
    const cookie = (page.data.peek as number | null | undefined) ?? null;
    return peekWidth(this.dragged === undefined ? cookie : this.dragged);
  }
}

export const peek = new PeekState();

// Opening a related ship from the bottom of a ship page. The ship is built
// on the stage while its card waits, pinned exactly over the card's
// thumbnail (the same camera the thumbnail was rendered with), so once it
// has loaded the picture swaps for the live 3D ship without a visible
// change. Then the ship pops out of the card to the middle of the screen as
// the page fades away behind it, the new page loads under that cover, and
// the cover lifts with the ship already in place. ship-leap.svelte draws the
// cover; the stage lifts above it meanwhile (+layout.svelte).
import { tick } from 'svelte';
import { goto } from '$app/navigation';
import { THUMBNAIL_INSET } from './components/ship-thumbnail.svelte';
import { stage } from './stage.svelte';
import type { ShipSummary } from './types';
import { TRANSITION_MS } from './viewer/poses';

/** How long the ship takes to pop out to the middle and the page to fade, in ms. */
export const POP_MS = TRANSITION_MS;
/** How long the cover takes to lift off the new page, in ms. */
export const LAND_MS = 700;
/** Room for the new page to lay out and pick its chapter before the cover lifts, in ms. */
const SETTLE_MS = 120;
/** Give up on the pinned ship after this long, in ms, and just open the page. */
const LOAD_TIMEOUT = 10000;

class Leap {
  /** While a leap runs: loading the ship over its card, popping it out, or landing. */
  phase = $state<'load' | 'pop' | 'land' | null>(null);
}

export const leap = new Leap();

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Resolves once `done` holds, checked every frame, or after `timeout` ms. */
function until(done: () => boolean, timeout: number) {
  const start = performance.now();
  return new Promise<void>((resolve) => {
    const check = () => {
      if (done() || performance.now() - start > timeout) resolve();
      else requestAnimationFrame(check);
    };
    check();
  });
}

/**
 * Handles a click on a ship card: a plain click leaps to the ship. Modified
 * clicks (new tab and so on), cards without one of our thumbnails, reduced
 * motion and a stage without an engine are left to the link.
 */
export async function leapTo(ship: ShipSummary, event: MouseEvent) {
  const plain =
    event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
  const picture = (event.currentTarget as HTMLElement).querySelector<HTMLImageElement>(
    'img[data-thumbnail]',
  );
  if (
    !plain ||
    !picture?.complete ||
    leap.phase ||
    !stage.releasePin ||
    matchMedia('(prefers-reduced-motion: reduce)').matches
  ) {
    stage.morphId = ship.id;
    return;
  }
  event.preventDefault();

  // The picture's square, without the room around the ship. Its hover lift
  // stays as it is, so the ship lands where the picture is now.
  const box = picture.getBoundingClientRect();
  const inset = box.width * THUMBNAIL_INSET;
  const rect = {
    x: box.x + inset,
    y: box.y + inset,
    width: box.width - inset * 2,
    height: box.height - inset * 2,
  };
  picture.style.transform = getComputedStyle(picture).transform;
  picture.style.transition = 'none';

  leap.phase = 'load';
  stage.leap = { rect, shown: false };
  stage.show(ship);
  try {
    // The stage takes the new ship up (its status then follows that ship).
    await tick();
    await until(
      () => stage.ship?.id === ship.id && (stage.status === 'ready' || stage.status === 'error'),
      LOAD_TIMEOUT,
    );
    if (stage.status === 'ready' && stage.leap) {
      // The live ship takes the picture's place, then pops out.
      stage.leap.shown = true;
      picture.style.visibility = 'hidden';
      leap.phase = 'pop';
      stage.releasePin(POP_MS);
      await wait(POP_MS);
    }
    await goto(`/ships/${ship.id}`);
    await wait(SETTLE_MS);
    leap.phase = 'land';
    await wait(LAND_MS);
  } finally {
    stage.leap = null;
    leap.phase = null;
    picture.style.removeProperty('transform');
    picture.style.removeProperty('transition');
    picture.style.removeProperty('visibility');
  }
}

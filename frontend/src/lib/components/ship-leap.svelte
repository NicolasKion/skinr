<script lang="ts">
  // The cover a leap to another ship runs under ($lib/leap.svelte.ts). While
  // the ship loads over its card it only holds the page still; as the ship
  // pops out it fades the page away, and it lifts off the new page at the
  // end. Below the site bar (z-40) and the lifted stage, above the page.
  import { LAND_MS, POP_MS, leap } from '$lib/leap.svelte';

  /** Keeps the page from scrolling under the card while its ship loads. */
  function holdScroll(node: HTMLElement) {
    const stop = (event: Event) => event.preventDefault();
    node.addEventListener('wheel', stop, { passive: false });
    node.addEventListener('touchmove', stop, { passive: false });
    return {
      destroy() {
        node.removeEventListener('wheel', stop);
        node.removeEventListener('touchmove', stop);
      },
    };
  }
</script>

{#if leap.phase}
  <div
    class="leap fixed inset-0 z-30 {leap.phase}"
    style:--pop="{POP_MS}ms"
    style:--land="{LAND_MS}ms"
    use:holdScroll
  ></div>
{/if}

<style>
  .leap {
    background: var(--background);
    opacity: 0;
  }
  .leap.load {
    cursor: progress;
  }
  .leap.pop {
    animation: cover-in var(--pop) var(--ease-apple) both;
  }
  .leap.land {
    opacity: 1;
    animation: cover-out var(--land) var(--ease-apple) both;
  }
  @keyframes cover-in {
    to {
      opacity: 1;
    }
  }
  @keyframes cover-out {
    to {
      opacity: 0;
    }
  }
</style>

<script lang="ts">
  // The preview panel peeking in from the right edge: the shared 3D stage
  // docks into its frame, the page's labels sit above it, and its left edge
  // can be dragged to make it wider (the root layout holds the width as
  // `--peek`, so every page's panel is the same). The frame and the labels
  // are two fixed layers around the canvas (z-10): a fixed element is its own
  // stacking context, so they cannot share one. The frame runs past the right
  // edge so only its left corners show. Between pages that both have one, the
  // panel stays put while the page changes (layout.css, `data-peek-static`).
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import { PEEK_KEY_STEP, PEEK_MAX_SHARE, PEEK_MIN_REM, savePeek } from '#lib/peek.js';
  import { peek } from '#lib/peek.svelte.js';
  import { dockTarget } from '#lib/stage.svelte.js';

  let {
    tint,
    backdrop,
    children,
  }: {
    /** The race color the frame glows in. */
    tint: string;
    /** Drawn in the frame, behind the ship (a faction emblem). */
    backdrop?: Snippet;
    /** The labels and actions over the ship; none, no labels layer. */
    children?: Snippet;
  } = $props();

  /**
   * How much of the site footer has scrolled into view, in px. The panel is
   * fixed to the viewport, so it ends that much higher instead of running
   * under the footer.
   */
  let footerIn = $state(0);
  function measureFooter() {
    const footer = document.querySelector('[data-site-footer]');
    const top = footer?.getBoundingClientRect().top ?? Infinity;
    footerIn = Math.max(0, Math.round(window.innerHeight - top));
  }
  // The page's height moves the footer too (lists filtering, loading more).
  $effect(() => {
    measureFooter();
    const observer = new ResizeObserver(measureFooter);
    observer.observe(document.body);
    return () => observer.disconnect();
  });

  /** The share in effect: dragged this visit, else the one the server rendered. */
  const share = $derived(
    peek.dragged === undefined
      ? ((page.data.peek as number | null | undefined) ?? null)
      : peek.dragged,
  );
  let handle = $state<HTMLElement>();
  /** The share the panel has on screen, the default included (the handle's value). */
  let shownShare = $state(0);
  function measureShare() {
    if (handle) shownShare = currentShare(handle);
  }
  // A share set by dragging is the one on screen. Only the default needs
  // measuring, and that waits for the next frame: reading the layout right
  // after the width changed would make the browser lay out the whole page
  // again there and then.
  $effect(() => {
    if (share !== null) {
      shownShare = share;
      return;
    }
    const frame = requestAnimationFrame(measureShare);
    return () => cancelAnimationFrame(frame);
  });

  /** A share inside the bounds the CSS clamp keeps, so the handle stays under the pointer. */
  function clampShare(value: number) {
    const rem = parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
    const min = (PEEK_MIN_REM * rem) / window.innerWidth;
    return Math.min(PEEK_MAX_SHARE, Math.max(min, value));
  }
  /**
   * The panel's width for an edge at `x`, as a share of the viewport. `vw`
   * counts the scrollbar but the panel's right edge (100% of a fixed box)
   * stops short of it, so the width runs from `x` to the scrollbar.
   */
  function shareAt(x: number) {
    return (document.documentElement.clientWidth - x) / window.innerWidth;
  }
  /** The share the panel has on screen: right of the handle's centre. */
  function currentShare(element: HTMLElement) {
    const box = element.getBoundingClientRect();
    return shareAt(box.left + box.width / 2);
  }
  // The drag follows the pointer on the window rather than through pointer
  // capture, which Chrome can drop mid-drag as the panel's edge moves away.
  // Pointer events can come several times a frame; the width (and so the
  // page's layout) changes at most once per frame, to the latest position.
  let pointerX: number | null = null;
  let resizeFrame = 0;
  function applyResize() {
    resizeFrame = 0;
    if (pointerX !== null) peek.dragged = clampShare(shareAt(pointerX));
  }
  function moveResize(event: PointerEvent) {
    pointerX = event.clientX;
    resizeFrame ||= requestAnimationFrame(applyResize);
  }
  function endResize() {
    window.removeEventListener('pointermove', moveResize);
    window.removeEventListener('pointerup', endResize);
    window.removeEventListener('pointercancel', endResize);
    // The last position counts even if its frame has not come yet.
    if (resizeFrame) {
      cancelAnimationFrame(resizeFrame);
      applyResize();
    }
    if (!peek.resizing) return;
    peek.resizing = false;
    if (peek.dragged !== undefined) savePeek(peek.dragged);
  }
  function startResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    peek.resizing = true;
    window.addEventListener('pointermove', moveResize);
    window.addEventListener('pointerup', endResize);
    window.addEventListener('pointercancel', endResize);
  }
  function resetResize() {
    peek.dragged = null;
    savePeek(null);
  }
  function keyResize(event: KeyboardEvent) {
    const step = { ArrowLeft: PEEK_KEY_STEP, ArrowRight: -PEEK_KEY_STEP }[event.key];
    if (step === undefined) return;
    event.preventDefault();
    peek.dragged = clampShare((share ?? shownShare) + step);
    savePeek(peek.dragged);
  }
  // Dragging anywhere keeps the resize cursor and selects no text.
  $effect(() => {
    document.documentElement.classList.toggle('resizing-peek', peek.resizing);
  });
</script>

<svelte:window
  onscroll={measureFooter}
  onresize={() => {
    measureFooter();
    measureShare();
  }}
/>

<div
  class="peek peek-frame pointer-events-none fixed top-[4.5rem] right-[-3rem] z-0 hidden rounded-l-[2.25rem] lg:block"
  style:--dock-tint={tint}
  style:--peek={peek.css}
  style:--footer-in="{footerIn}px"
  use:dockTarget={'peek'}
>
  {@render backdrop?.()}
</div>

{#if children}
  <div
    class="peek peek-labels pointer-events-none fixed top-[4.5rem] right-0 z-20 hidden flex-col justify-between p-9 lg:flex"
    style:--peek={peek.css}
    style:--footer-in="{footerIn}px"
  >
    {@render children()}
  </div>
{/if}

<!-- The panel's left edge: drag it (or use the arrow keys) to make the panel
     wider or narrower, double-click it for the default width. -->
<!-- A focusable separator is the WAI-ARIA window splitter, an interactive
     role Svelte's checks do not count as one. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="peek peek-handle fixed top-[4.5rem] z-20 hidden w-4 -translate-x-1/2 cursor-col-resize touch-none lg:block"
  class:active={peek.resizing}
  style:--peek={peek.css}
  style:--footer-in="{footerIn}px"
  role="separator"
  aria-orientation="vertical"
  aria-label="Resize the preview"
  aria-valuemin={0}
  aria-valuemax={PEEK_MAX_SHARE * 100}
  aria-valuenow={Math.round(shownShare * 100)}
  tabindex="0"
  bind:this={handle}
  onpointerdown={startResize}
  ondblclick={resetResize}
  onkeydown={keyResize}
></div>

<style>
  .peek {
    left: calc(100% - var(--peek));
    /* Clear of the viewport's edge, and of the footer once it scrolls in. */
    bottom: calc(0.75rem + var(--footer-in, 0px));
  }
  .peek-frame {
    box-shadow:
      inset 1px 0 0 color-mix(in oklab, var(--foreground) 7%, transparent),
      -40px 0 80px -40px var(--shadow);
    background:
      radial-gradient(
        70% 55% at 40% 50%,
        color-mix(in oklab, var(--dock-tint) 20%, transparent),
        transparent 70%
      ),
      linear-gradient(
        90deg,
        color-mix(in oklab, var(--foreground) 5%, transparent),
        color-mix(in oklab, var(--foreground) 1.5%, transparent) 40%
      );
    transition: --dock-tint 1.3s var(--ease-apple);
  }
  /* Registered so the race glow fades from one race's color to the next. */
  @property --dock-tint {
    syntax: '<color>';
    inherits: false;
    initial-value: transparent;
  }
  /* A slim grip, centred on the panel's edge, that shows on hover and while dragging. */
  .peek-handle::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 4px;
    height: 3rem;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 35%, transparent);
    opacity: 0;
    translate: -50% -50%;
    transition:
      opacity 0.3s var(--ease-apple),
      height 0.3s var(--ease-apple);
  }
  .peek-handle:hover::after,
  .peek-handle:focus-visible::after,
  .peek-handle.active::after {
    opacity: 1;
    height: 4.5rem;
  }
  .peek-handle:focus-visible {
    outline: none;
  }
  :global(html.resizing-peek) {
    cursor: col-resize;
    user-select: none;
  }
</style>

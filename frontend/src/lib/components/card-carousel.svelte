<script lang="ts">
  // A row of cards under a heading (shadcn's Embla carousel): drag or swipe
  // it, or page through with the arrows beside the heading; each end fades
  // out while there is more to see that way. The caller draws the cards,
  // each in a Carousel.Item.
  //
  // A page is what is clear of the fades right now, so it follows the row's
  // width (the explorer's stage can be dragged wider): Next brings the first
  // card that is not fully shown to just inside the left fade, Prev goes
  // back by as many cards as are shown.
  import type { Snippet } from 'svelte';
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';
  import * as Carousel from '#lib/components/ui/carousel/index.js';
  import type { CarouselAPI } from '#lib/components/ui/carousel/context.js';
  import { peek } from '#lib/peek.svelte.js';

  let {
    label,
    heading,
    meta,
    children,
  }: {
    /** What the row holds, for the arrows' labels ("ships"). */
    label: string;
    heading: Snippet;
    /** Drawn before the arrows, at the heading's end (a count). */
    meta?: Snippet;
    children: Snippet;
  } = $props();

  let api = $state<CarouselAPI>();
  let canPrev = $state(false);
  let canNext = $state(false);

  /** How far each end fades, in rem (the mask below draws it). */
  const FADE_REM = 6;
  /** Slack when telling a card fully shown, in px, for subpixel layout. */
  const EDGE_SLACK = 1;

  function fadePx() {
    return FADE_REM * (parseFloat(getComputedStyle(document.documentElement).fontSize) || 16);
  }
  /** The cards, not their slides: each slide pads its card on the left. */
  function cards() {
    return (api?.slideNodes() ?? []).map(
      (slide) => (slide.firstElementChild as HTMLElement | null) ?? slide,
    );
  }
  /**
   * Where Embla puts a snapped slide, from where the container rests: so the
   * card inside lands just inside the left fade. The container rests offset
   * by its margin, and the card sits inside its slide's padding.
   */
  function align() {
    const container = api?.containerNode();
    const slide = api?.slideNodes()[0];
    if (!container || !slide) return fadePx();
    const margin = parseFloat(getComputedStyle(container).marginInlineStart) || 0;
    const inset = parseFloat(getComputedStyle(slide).paddingInlineStart) || 0;
    return fadePx() - margin - inset;
  }
  /** The part of the row clear of the fades in effect. */
  function clearArea() {
    const box = api?.rootNode().getBoundingClientRect();
    if (!box) return null;
    const fade = fadePx();
    return { left: box.left + (canPrev ? fade : 0), right: box.right - (canNext ? fade : 0) };
  }
  /** Scrolls to card `index`; the last cards share the final snap. */
  function scrollToCard(index: number) {
    if (!api) return;
    api.scrollTo(Math.max(0, Math.min(index, api.scrollSnapList().length - 1)));
  }
  function next() {
    const area = clearArea();
    if (!area) return;
    const boxes = cards().map((card) => card.getBoundingClientRect());
    const hidden = boxes.findIndex((box) => box.right > area.right + EDGE_SLACK);
    scrollToCard(hidden === -1 ? boxes.length - 1 : hidden);
  }
  function prev() {
    const area = clearArea();
    if (!area) return;
    const boxes = cards().map((card) => card.getBoundingClientRect());
    const shown = boxes.flatMap((box, index) =>
      box.left >= area.left - EDGE_SLACK && box.right <= area.right + EDGE_SLACK ? [index] : [],
    );
    const first = shown[0] ?? boxes.findIndex((box) => box.right > area.left);
    scrollToCard(first - Math.max(1, shown.length));
  }

  $effect(() => {
    if (!api) return;
    const update = () => {
      canPrev = api?.canScrollPrev() ?? false;
      canNext = api?.canScrollNext() ?? false;
    };
    // Embla aligned the snaps before it handed out the API, so without the
    // slides `align` could not see the card inset; lay them out again.
    api.reInit();
    update();
    api.on('select', update).on('scroll', update).on('reInit', update);
    return () => {
      api?.off('select', update).off('scroll', update).off('reInit', update);
    };
  });

  // While the preview panel is dragged the carousel's width changes every
  // frame, and re-measuring every card each time (watchResize below) is
  // what makes the drag stutter; it measures once when the drag ends.
  let wasResizing = false;
  $effect(() => {
    const resizing = peek.resizing;
    if (wasResizing && !resizing) api?.reInit();
    wasResizing = resizing;
  });
</script>

<div class="mb-5 flex items-end justify-between gap-6">
  <div class="min-w-0">{@render heading()}</div>
  <div class="flex shrink-0 items-center gap-3">
    {@render meta?.()}
    {#if canPrev || canNext}
      <div class="flex gap-1.5">
        <button
          type="button"
          class="arrow glass"
          aria-label="Previous {label}"
          disabled={!canPrev}
          onclick={prev}
        >
          <ChevronLeft class="size-4" />
        </button>
        <button
          type="button"
          class="arrow glass"
          aria-label="Next {label}"
          disabled={!canNext}
          onclick={next}
        >
          <ChevronRight class="size-4" />
        </button>
      </div>
    {/if}
  </div>
</div>
<Carousel.Root
  class={['card-carousel -mx-6', canPrev && 'fade-start', canNext && 'fade-end']}
  opts={{
    align,
    slidesToScroll: 1,
    containScroll: 'trimSnaps',
    dragFree: true,
    watchResize: () => !peek.resizing,
  }}
  setApi={(next) => (api = next)}
>
  <Carousel.Content class="px-6 pb-4">
    {@render children()}
  </Carousel.Content>
</Carousel.Root>

<style>
  /* Registered so the fades can transition rather than snap on and off. */
  @property --fade-start {
    syntax: '<length>';
    inherits: false;
    initial-value: 0px;
  }
  @property --fade-end {
    syntax: '<length>';
    inherits: false;
    initial-value: 0px;
  }
  /* On the carousel root, a child component, hence global. */
  :global(.card-carousel) {
    --fade: 6rem; /* FADE_REM */
    mask-image: linear-gradient(
      90deg,
      transparent,
      black var(--fade-start),
      black calc(100% - var(--fade-end)),
      transparent
    );
    transition:
      --fade-start 0.4s var(--ease-apple),
      --fade-end 0.4s var(--ease-apple);
  }
  :global(.card-carousel.fade-start) {
    --fade-start: var(--fade);
  }
  :global(.card-carousel.fade-end) {
    --fade-end: var(--fade);
  }
  .arrow {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 999px;
    color: color-mix(in oklab, var(--foreground) 85%, transparent);
    transition:
      opacity 0.3s,
      transform 0.3s var(--ease-apple);
  }
  .arrow:hover:not(:disabled) {
    transform: scale(1.08);
  }
  .arrow:disabled {
    opacity: 0.3;
  }
</style>

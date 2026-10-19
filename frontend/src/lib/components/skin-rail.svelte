<script lang="ts" generics="T extends { key: string; name: string }">
  // A rail of SKIN hexagons for the livery chapter: the game's own skins or
  // the players' SKINR designs. It keeps the chosen one in its middle (the
  // first few stay at its start), so arrow keys stepping through always show
  // where they are; the arrow buttons page through a few at a time.
  import type { Snippet } from 'svelte';
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';
  import * as Carousel from '#lib/components/ui/carousel/index.js';
  import type { CarouselAPI } from '#lib/components/ui/carousel/context.js';

  let {
    label,
    items,
    selected,
    onselect,
    icon,
  }: {
    /** What the rail holds, above it with the count ("In-game", "SKINR"). */
    label: string;
    items: T[];
    /** The key of the item on the ship, when it is in this rail. */
    selected: string | null;
    onselect: (item: T) => void;
    /** Draws an item's hexagon. */
    icon: Snippet<[T, boolean]>;
  } = $props();

  /** How many items the rail's arrow buttons move it by. */
  const ARROW_STEP = 5;

  let api = $state<CarouselAPI>();
  let canPrev = $state(false);
  let canNext = $state(false);
  $effect(() => {
    if (!api) return;
    const update = () => {
      canPrev = api?.canScrollPrev() ?? false;
      canNext = api?.canScrollNext() ?? false;
    };
    update();
    api.on('select', update).on('scroll', update).on('reInit', update);
    return () => {
      api?.off('select', update).off('scroll', update).off('reInit', update);
    };
  });

  /** The arrow buttons page through the rail, a few items at a time. */
  function scrollBy(step: 1 | -1) {
    if (!api) return;
    const last = api.scrollSnapList().length - 1;
    api.scrollTo(Math.min(last, Math.max(0, api.selectedScrollSnap() + step * ARROW_STEP)));
  }

  // Keep the chosen item in the middle of the rail. Snaps are not slides:
  // trimming the ends folds the first and last few slides into one snap
  // each, so look the slide's snap up.
  $effect(() => {
    const index = items.findIndex((item) => item.key === selected);
    if (!api || index < 0) return;
    const snap = api.internalEngine().slideRegistry.findIndex((slides) => slides.includes(index));
    if (snap >= 0) api.scrollTo(snap);
  });
</script>

<div class="reveal">
  <div class="mb-1 flex items-center justify-between">
    <p class="text-xs text-muted-foreground">
      <span class="font-medium text-foreground">{label}</span>
      <span class="ml-1.5 tabular-nums">{items.length}</span>
    </p>
    {#if canPrev || canNext}
      <div class="flex gap-1.5">
        <button
          type="button"
          class="arrow glass"
          aria-label="Previous {label}"
          disabled={!canPrev}
          onclick={() => scrollBy(-1)}
        >
          <ChevronLeft class="size-4" />
        </button>
        <button
          type="button"
          class="arrow glass"
          aria-label="Next {label}"
          disabled={!canNext}
          onclick={() => scrollBy(1)}
        >
          <ChevronRight class="size-4" />
        </button>
      </div>
    {/if}
  </div>
  <Carousel.Root
    class={['skin-rail', canPrev && 'fade-start', canNext && 'fade-end']}
    opts={{ align: 'center', containScroll: 'trimSnaps', dragFree: true }}
    setApi={(next) => (api = next)}
  >
    <Carousel.Content class="-ml-1 py-3">
      {#each items as item (item.key)}
        <Carousel.Item class="basis-auto pl-1">
          <button
            type="button"
            class="item block px-1"
            title={item.name}
            aria-label={item.name}
            aria-pressed={selected === item.key}
            onclick={(event) => {
              // A mouse click lets go of focus, or the next arrow key would
              // ring this item while the selection moves on. Keyboard clicks
              // (detail 0) keep it.
              if (event.detail > 0) event.currentTarget.blur();
              onselect(item);
            }}
          >
            {@render icon(item, selected === item.key)}
          </button>
        </Carousel.Item>
      {/each}
    </Carousel.Content>
  </Carousel.Root>
</div>

<style>
  .item :global(.skin-icon) {
    transition: transform 0.5s var(--ease-apple);
  }
  .item:hover :global(.skin-icon:not(.selected)) {
    transform: translateY(-3px);
  }
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
  :global(.skin-rail) {
    --fade: 4rem;
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
  :global(.skin-rail.fade-start) {
    --fade-start: var(--fade);
  }
  :global(.skin-rail.fade-end) {
    --fade-end: var(--fade);
  }
  .arrow {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 999px;
    color: color-mix(in oklab, var(--foreground) 85%, transparent);
    transition: opacity 0.3s;
  }
  .arrow:disabled {
    opacity: 0.3;
  }
</style>

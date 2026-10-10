<script lang="ts">
  // The SKINR chapter's picker: the chosen player design in one line (its
  // name, who made it, its price), then every design listed for this hull as
  // a rail of SKIN hexagons that keeps the chosen one in its middle, so arrow
  // keys stepping through the designs always show where they are.
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';
  import * as Carousel from '$lib/components/ui/carousel';
  import type { CarouselAPI } from '$lib/components/ui/carousel/context';
  import SkinIcon from '$lib/components/skin-icon.svelte';
  import { skinrIcon } from '$lib/skinr';
  import type { SkinrComponent, SkinrDesign } from '$lib/types';

  let {
    designs,
    components,
    selected,
    onselect,
  }: {
    designs: SkinrDesign[];
    components: Record<string, SkinrComponent>;
    selected: SkinrDesign | null;
    onselect: (design: SkinrDesign) => void;
  } = $props();

  const number = new Intl.NumberFormat('en-US');
  /** How many designs the rail's arrow buttons move it by. */
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

  /** The arrow buttons page through the rail, a few designs at a time. */
  function scrollBy(step: 1 | -1) {
    if (!api) return;
    const last = api.scrollSnapList().length - 1;
    api.scrollTo(Math.min(last, Math.max(0, api.selectedScrollSnap() + step * ARROW_STEP)));
  }

  // Keep the chosen design in the middle of the rail (the first few stay at
  // its start). Snaps are not slides: trimming the ends folds the first and
  // last few slides into one snap each, so look the slide's snap up.
  $effect(() => {
    const index = designs.findIndex((design) => design.id === selected?.id);
    if (!api || index < 0) return;
    const snap = api.internalEngine().slideRegistry.findIndex((slides) => slides.includes(index));
    if (snap >= 0) api.scrollTo(snap);
  });
</script>

<div class="reveal mb-6 min-h-14">
  {#if selected}
    <p class="truncate text-2xl font-semibold tracking-tight">{selected.name}</p>
    <p class="mt-1 truncate text-sm text-muted-foreground">
      by <span class="text-foreground">{selected.creator.name ?? 'a capsuleer'}</span
      >{#if selected.min_price_plex !== null}{` · ${number.format(selected.min_price_plex)} PLEX`}{/if}
    </p>
  {:else}
    <p class="text-muted-foreground">Pick a design to see it on the hull.</p>
  {/if}
</div>

<div class="reveal">
  <div class="mb-2 flex items-center justify-between">
    <p class="text-xs text-muted-foreground tabular-nums">
      {designs.length} designs
    </p>
    {#if canPrev || canNext}
      <div class="flex gap-1.5">
        <button
          type="button"
          class="arrow glass"
          aria-label="Previous designs"
          disabled={!canPrev}
          onclick={() => scrollBy(-1)}
        >
          <ChevronLeft class="size-4" />
        </button>
        <button
          type="button"
          class="arrow glass"
          aria-label="Next designs"
          disabled={!canNext}
          onclick={() => scrollBy(1)}
        >
          <ChevronRight class="size-4" />
        </button>
      </div>
    {/if}
  </div>
  <Carousel.Root
    class={['design-rail', canPrev && 'fade-start', canNext && 'fade-end']}
    opts={{ align: 'center', containScroll: 'trimSnaps', dragFree: true }}
    setApi={(next) => (api = next)}
  >
    <Carousel.Content class="-ml-1 py-3">
      {#each designs as design (design.id)}
        {@const icon = skinrIcon(design, components)}
        <Carousel.Item class="basis-auto pl-1">
          <button
            type="button"
            class="design block px-1"
            title={design.name}
            aria-label={design.name}
            aria-pressed={selected?.id === design.id}
            onclick={() => onselect(design)}
          >
            <SkinIcon
              colors={icon.colors}
              patterns={icon.patterns}
              selected={selected?.id === design.id}
              class="size-14"
            />
          </button>
        </Carousel.Item>
      {/each}
    </Carousel.Content>
  </Carousel.Root>
</div>

<style>
  .design :global(.skin-icon) {
    transition: transform 0.5s var(--ease-apple);
  }
  .design:hover :global(.skin-icon:not(.selected)) {
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
  :global(.design-rail) {
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
  :global(.design-rail.fade-start) {
    --fade-start: var(--fade);
  }
  :global(.design-rail.fade-end) {
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

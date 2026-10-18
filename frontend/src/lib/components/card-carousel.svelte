<script lang="ts">
  // A row of cards under a heading (shadcn's Embla carousel): drag or swipe
  // it, or page through with the arrows beside the heading; each end fades
  // out while there is more to see that way. The caller draws the cards,
  // each in a Carousel.Item.
  import type { Snippet } from 'svelte';
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';
  import * as Carousel from '$lib/components/ui/carousel';
  import type { CarouselAPI } from '$lib/components/ui/carousel/context';

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
          onclick={() => api?.scrollPrev()}
        >
          <ChevronLeft class="size-4" />
        </button>
        <button
          type="button"
          class="arrow glass"
          aria-label="Next {label}"
          disabled={!canNext}
          onclick={() => api?.scrollNext()}
        >
          <ChevronRight class="size-4" />
        </button>
      </div>
    {/if}
  </div>
</div>
<Carousel.Root
  class={['card-carousel -mx-6', canPrev && 'fade-start', canNext && 'fade-end']}
  opts={{ align: 'start', slidesToScroll: 'auto', containScroll: 'trimSnaps', dragFree: true }}
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
    --fade: 6rem;
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

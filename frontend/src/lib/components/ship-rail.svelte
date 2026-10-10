<script lang="ts">
  // One row of related ships: a caption, a line on why they belong together,
  // and a carousel of cards (shadcn's Embla carousel): drag or swipe it, or
  // page through with the arrows; each end fades out while there is more to
  // see that way. Each card shows the ship's thumbnail with a halo
  // in the ship's livery color on hover, its meta group as a badge when it is
  // anything but Tech I, and its class and race.
  import { ChevronLeft, ChevronRight } from '@lucide/svelte';
  import ShipThumbnail from '$lib/components/ship-thumbnail.svelte';
  import * as Carousel from '$lib/components/ui/carousel';
  import type { CarouselAPI } from '$lib/components/ui/carousel/context';
  import { raceTint } from '$lib/eve';
  import type { MetaGroup, ShipSummary } from '$lib/types';

  let {
    title,
    note,
    ships,
    groupNames,
    raceNames,
    metaGroups,
    factionLogos,
    showClass = true,
    onpick,
  }: {
    title: string;
    note: string;
    ships: ShipSummary[];
    groupNames: Map<number, string>;
    raceNames: Map<number, string>;
    metaGroups: Map<number, MetaGroup>;
    /** Each faction's emblem, by faction id, watermarked behind its ships. */
    factionLogos: Map<number, string | null>;
    /** Whether cards name their class; a rail of one class leaves it out. */
    showClass?: boolean;
    /** Called with the ship a card opens, before the navigation. */
    onpick: (ship: ShipSummary) => void;
  } = $props();

  /** The SDE's Tech I meta group: the plain hulls, which get no badge. */
  const TECH_ONE = 1;
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

<section class="reveal">
  <div class="mb-5 flex items-baseline justify-between gap-6">
    <div>
      <h3 class="text-xl font-semibold tracking-tight">{title}</h3>
      <p class="mt-1 text-sm text-muted-foreground">{note}</p>
    </div>
    <div class="flex shrink-0 items-center gap-3">
      <p class="text-xs text-muted-foreground tabular-nums">{ships.length} ships</p>
      {#if canPrev || canNext}
        <div class="flex gap-1.5">
          <button
            type="button"
            class="arrow glass"
            aria-label="Previous ships"
            disabled={!canPrev}
            onclick={() => api?.scrollPrev()}
          >
            <ChevronLeft class="size-4" />
          </button>
          <button
            type="button"
            class="arrow glass"
            aria-label="Next ships"
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
    class={['ship-rail -mx-6', canPrev && 'fade-start', canNext && 'fade-end']}
    opts={{ align: 'start', slidesToScroll: 'auto', containScroll: 'trimSnaps', dragFree: true }}
    setApi={(next) => (api = next)}
  >
    <Carousel.Content class="px-6 pb-4">
      {#each ships as other (other.id)}
        {@const meta = other.meta_group_id != null ? metaGroups.get(other.meta_group_id) : null}
        <Carousel.Item class="basis-auto">
          <a
            href="/ships/{other.id}"
            class="group block w-44"
            style:--glow={other.colors.primary ?? raceTint(other.race_id)}
            onclick={() => onpick(other)}
          >
            <div class="relative">
              <div
                class="absolute inset-4 rounded-full opacity-0 blur-2xl transition-opacity duration-700 group-hover:opacity-60"
                style:background="var(--glow)"
              ></div>
              <div
                class="relative aspect-square overflow-hidden rounded-[1.5rem] bg-foreground/[0.03] shadow-[0_30px_60px_-30px_var(--shadow),inset_0_0_0_1px_color-mix(in_oklab,var(--foreground)_4%,transparent)] transition-transform duration-500 ease-[var(--ease-apple)]"
              >
                <ShipThumbnail
                  id={other.id}
                  raceId={other.race_id}
                  logoUrl={other.faction_id != null ? factionLogos.get(other.faction_id) : null}
                  class="h-full w-full transition-transform duration-700 ease-[var(--ease-apple)] group-hover:-translate-y-[3%] group-hover:scale-[1.1]"
                />
                {#if meta && meta.id !== TECH_ONE}
                  <span
                    class="glass absolute top-2.5 left-2.5 flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[0.65rem] font-medium"
                  >
                    <span class="size-1.5 rounded-full" style:background={meta.color ?? 'white'}
                    ></span>
                    {meta.name}
                  </span>
                {/if}
              </div>
            </div>
            <p class="mt-3 truncate text-sm font-medium">{other.name}</p>
            <p class="truncate text-xs text-muted-foreground">
              {#if showClass}{groupNames.get(other.group_id) ?? ''} ·
              {/if}{raceNames.get(other.race_id ?? -1) ?? 'Unaffiliated'}
            </p>
          </a>
        </Carousel.Item>
      {/each}
    </Carousel.Content>
  </Carousel.Root>
</section>

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
  :global(.ship-rail) {
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
  :global(.ship-rail.fade-start) {
    --fade-start: var(--fade);
  }
  :global(.ship-rail.fade-end) {
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

<script lang="ts">
  // One row of related ships: a caption, a line on why they belong together,
  // and a carousel of cards. Each card shows the ship's thumbnail with a halo
  // in the ship's livery color on hover, its meta group as a badge when it is
  // anything but Tech I, and its class and race.
  import CardCarousel from '$lib/components/card-carousel.svelte';
  import ShipThumbnail from '$lib/components/ship-thumbnail.svelte';
  import * as Carousel from '$lib/components/ui/carousel';
  import { raceTint } from '$lib/eve';
  import type { MetaGroup, ShipSummary } from '$lib/types';

  let {
    title,
    note,
    ships,
    groupNames,
    builderOf,
    metaGroups,
    factionLogos,
    showClass = true,
    onpick,
  }: {
    title: string;
    note: string;
    ships: ShipSummary[];
    groupNames: Map<number, string>;
    /** Who builds a ship: its race, or the faction for one outside its race's empire. */
    builderOf: (ship: ShipSummary) => string;
    metaGroups: Map<number, MetaGroup>;
    /** Each faction's emblem, by faction id, watermarked behind its ships. */
    factionLogos: Map<number, string | null>;
    /** Whether cards name their class; a rail of one class leaves it out. */
    showClass?: boolean;
    /** Called with the ship a card opens and the click, before the navigation. */
    onpick: (ship: ShipSummary, event: MouseEvent) => void;
  } = $props();

  /** The SDE's Tech I meta group: the plain hulls, which get no badge. */
  const TECH_ONE = 1;
</script>

<section class="reveal">
  <CardCarousel label="ships">
    {#snippet heading()}
      <h3 class="text-xl font-semibold tracking-tight">{title}</h3>
      <p class="mt-1 text-sm text-muted-foreground">{note}</p>
    {/snippet}
    {#snippet meta()}
      <p class="text-xs text-muted-foreground tabular-nums">{ships.length} ships</p>
    {/snippet}
    {#each ships as other (other.id)}
      {@const badge = other.meta_group_id != null ? metaGroups.get(other.meta_group_id) : null}
      <Carousel.Item class="basis-auto">
        <a
          href="/ships/{other.id}"
          class="group block w-44"
          style:--glow={other.colors.primary ?? raceTint(other.race_id)}
          onclick={(event) => onpick(other, event)}
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
              {#if badge && badge.id !== TECH_ONE}
                <span
                  class="glass absolute top-2.5 left-2.5 flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[0.65rem] font-medium"
                >
                  <span class="size-1.5 rounded-full" style:background={badge.color ?? 'white'}
                  ></span>
                  {badge.name}
                </span>
              {/if}
            </div>
          </div>
          <p class="mt-3 truncate text-sm font-medium">{other.name}</p>
          <p class="truncate text-xs text-muted-foreground">
            {#if showClass}{groupNames.get(other.group_id) ?? ''} ·
            {/if}{builderOf(other)}
          </p>
        </a>
      </Carousel.Item>
    {/each}
  </CardCarousel>
</section>

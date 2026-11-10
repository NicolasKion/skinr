<script lang="ts">
  // The page's end: more ships to open, in rails from closest to loosest.
  // A card leaps to its ship (#lib/leap.svelte.ts).
  import ShipRail from '#lib/components/ship-rail.svelte';
  import { leapTo } from '#lib/leap.svelte.js';
  import type { Catalog, ShipSummary } from '#lib/types.js';
  import { whenCrossing } from './chapters';
  import type { RelatedRail } from './related';

  let {
    rails,
    catalog,
    builderOf,
    tint,
    onenter,
  }: {
    rails: RelatedRail[];
    catalog: Catalog;
    /** Who builds a ship, for its card. */
    builderOf: (ship: ShipSummary) => string;
    tint: string;
    onenter: () => void;
  } = $props();

  const groupNames = $derived(new Map(catalog.groups.map((group) => [group.id, group.name])));
  const metaGroups = $derived(new Map(catalog.meta_groups.map((meta) => [meta.id, meta])));
  const factionLogos = $derived(
    new Map(catalog.factions.map((faction) => [faction.id, faction.logo_url])),
  );
</script>

<section id="related" class="mx-auto max-w-[90rem] px-6 pt-[20vh] pb-24" use:whenCrossing={onenter}>
  <p class="reveal eyebrow mb-4" style:color={tint}>Keep browsing</p>
  <h2 class="reveal display mb-14 text-[clamp(2.25rem,4vw,3.5rem)]">
    <span class="text-sheen">More hulls to dress up.</span>
  </h2>
  <div class="space-y-14">
    {#each rails as rail (rail.title)}
      <ShipRail
        title={rail.title}
        note={rail.note}
        ships={rail.ships}
        {groupNames}
        {builderOf}
        {metaGroups}
        {factionLogos}
        showClass={rail.showClass}
        onpick={leapTo}
      />
    {/each}
  </div>
</section>

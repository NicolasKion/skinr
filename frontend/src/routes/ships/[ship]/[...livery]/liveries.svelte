<script lang="ts">
  // The livery chapter: the ship in its own view (so the main one can stay
  // in the blueprint), and two rails to dress it from: the game's own skins
  // and the players' SKINR designs.
  import { page } from '$app/state';
  import SectionStage from '#lib/components/section-stage.svelte';
  import SkinIcon from '#lib/components/skin-icon.svelte';
  import SkinRail from '#lib/components/skin-rail.svelte';
  import ThumbnailDebug from '#lib/components/thumbnail-debug.svelte';
  import { skinrIcon } from '#lib/skinr.js';
  import type { ShipDetail, ShipSkinrs } from '#lib/types.js';
  import { designerHref } from '#lib/urls.js';
  import { FRONT_POSE } from '#lib/viewer/poses.js';
  import Chapter from './chapter.svelte';
  import { RIGHT } from './chapters';
  import type { LiveryState } from './livery.svelte';

  let {
    ship,
    skinrs,
    tint,
    wide,
    livery,
    card,
    onenter,
  }: {
    ship: ShipDetail;
    skinrs: ShipSkinrs;
    tint: string;
    /** Whether the text sits beside the ship. */
    wide: boolean;
    livery: LiveryState;
    /** The page's share card (the debug tools show it). */
    card: string;
    onenter: () => void;
  } = $props();

  const skinKey = (id: number | null) => `skin-${id ?? 'standard'}`;
  const skinItems = $derived(
    livery.options.map((option) => ({ key: skinKey(option.id), name: option.name, option })),
  );
  const designItems = $derived(
    skinrs.designs.map((design) => ({ key: design.id, name: design.name, design })),
  );
  const design = $derived(livery.design);
  const plex = new Intl.NumberFormat('en-US');
</script>

<Chapter id="liveries" class="relative" {onenter}>
  {#snippet stage()}
    <SectionStage
      key="liveries"
      pose={FRONT_POSE}
      {tint}
      logoUrl={ship.faction?.logo_url ?? null}
      {wide}
      placement={RIGHT}
    />
  {/snippet}
  <p class="reveal eyebrow mb-4" style:color={tint}>Skins</p>
  <h2 class="reveal display mb-4 text-[clamp(2.25rem,4vw,3.5rem)]">
    <span class="text-sheen"
      >{livery.options.length + skinrs.designs.length} looks. Try them on.</span
    >
  </h2>
  <p class="reveal mb-10 max-w-md text-muted-foreground">
    Every skin CCP makes for the {ship.name}{#if skinrs.designs.length}, and every design capsuleers
      have up for sale on the Paragon Hub{/if}. See it on the hull before you spend a single PLEX.
  </p>

  <!-- The look on the ship, from either rail. -->
  <div class="reveal mb-6 min-h-14">
    <div class="flex items-center gap-2">
      <p class="truncate text-2xl font-semibold tracking-tight">
        {design?.name ?? livery.name}
      </p>
      {#if page.data.debug}
        <ThumbnailDebug livery={livery.onShip} {card} />
      {/if}
    </div>
    <p class="mt-1 truncate text-sm text-muted-foreground">
      {#if design}
        SKINR by <a
          href={designerHref(design.creator)}
          class="text-foreground underline-offset-4 hover:underline"
          >{design.creator.name ?? 'a capsuleer'}</a
        >{#if design.min_price_plex !== null}{` · from ${plex.format(design.min_price_plex)} PLEX`}{/if}
      {:else if livery.skin}
        In-game skin
      {:else}
        The hull's own colors
      {/if}
    </p>
  </div>

  <div class="space-y-4">
    <SkinRail
      label="In-game"
      items={skinItems}
      selected={design ? null : skinKey(livery.skin?.id ?? null)}
      onselect={(item) => livery.pickSkin(item.option.id)}
    >
      {#snippet icon(item, selected)}
        <SkinIcon
          iconUrl={item.option.icon_url}
          iconColors={item.option.icon_colors}
          colors={item.option.colors}
          {selected}
          class="size-14"
        />
      {/snippet}
    </SkinRail>
    {#if designItems.length}
      <SkinRail
        label="SKINR"
        items={designItems}
        selected={design?.id ?? null}
        onselect={(item) => livery.pickDesign(item.design)}
      >
        {#snippet icon(item, selected)}
          {@const look = skinrIcon(item.design, skinrs.components)}
          <SkinIcon colors={look.colors} patterns={look.patterns} {selected} class="size-14" />
        {/snippet}
      </SkinRail>
    {/if}
  </div>
</Chapter>

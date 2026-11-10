<script lang="ts">
  // The SKINR pages' preview: the shared panel (the explorer's too), wearing
  // the picked design. Its labels name the design, and a link opens it on the
  // ship's own page. The page keeps its content clear of it with
  // `lg:pr-[calc(var(--peek)+2rem)]` with `style:--peek={peek.css}`.
  import { designHref } from '#lib/urls.js';
  import { ArrowRight, LoaderCircle } from '@lucide/svelte';
  import { fade } from 'svelte/transition';
  import FactionEmblem from '#lib/components/faction-emblem.svelte';
  import { isPlainClick } from '#lib/clicks.js';
  import PeekPanel from '#lib/components/peek-panel.svelte';
  import StageAlert from '#lib/components/stage-alert.svelte';
  import { raceTint } from '#lib/eve.js';
  import { stage } from '#lib/stage.svelte.js';
  import type { ShipSummary, SkinrDesign } from '#lib/types.js';

  let {
    design,
    ship,
    logoUrl,
    note = null,
  }: {
    design: SkinrDesign | null;
    /** The design's hull, from the catalog. */
    ship: ShipSummary | null;
    logoUrl: string | null;
    /** A line under the name: a price, licenses owned. */
    note?: string | null;
  } = $props();

  const tint = $derived(raceTint(ship?.race_id));
</script>

<PeekPanel {tint}>
  {#snippet backdrop()}
    {#key logoUrl}
      <div
        class="absolute top-1/2 left-[42%] size-[min(48rem,70vh)] -translate-x-1/2 -translate-y-1/2"
        transition:fade={{ duration: 900 }}
      >
        {#if logoUrl}
          <FactionEmblem url={logoUrl} {tint} opacity={0.1} class="size-full" />
        {/if}
      </div>
    {/key}
  {/snippet}
  <div class="grid">
    {#if design}
      {#key design.id}
        <div class="[grid-area:1/1]" transition:fade={{ duration: 500 }}>
          {#if ship}
            <StageAlert name={ship.name} class="pointer-events-auto mb-6" />
          {/if}
          <p class="eyebrow" style:color={tint}>
            {[ship?.name, design.creator.name && `by ${design.creator.name}`]
              .filter(Boolean)
              .join(' · ')}
          </p>
          <p class="display mt-3 text-[clamp(2.25rem,3.5vw,3.5rem)]">{design.name}</p>
          {#if note}
            <p class="mt-3 text-sm text-muted-foreground">{note}</p>
          {/if}
        </div>
      {/key}
    {/if}
  </div>
  {#if design && ship}
    <div class="flex items-center justify-between gap-4">
      <span class="flex items-center gap-2 text-xs text-muted-foreground">
        {#if stage.status === 'starting' || stage.status === 'loading'}
          <LoaderCircle class="size-3.5 animate-spin" />
          {stage.status === 'starting' ? 'Starting the engine' : 'Assembling'}
        {:else if stage.status === 'error'}
          Could not assemble
        {:else}
          Drag to rotate
        {/if}
      </span>
      <a
        href={designHref(ship, design)}
        class="pointer-events-auto flex items-center gap-1.5 rounded-full bg-foreground px-5 py-2.5 text-sm font-medium text-background transition-transform hover:scale-[1.03]"
        onclick={(event) => {
          if (isPlainClick(event)) stage.morphId = null;
        }}
      >
        On the {ship.name}
        <ArrowRight class="size-4" />
      </a>
    </div>
  {/if}
</PeekPanel>

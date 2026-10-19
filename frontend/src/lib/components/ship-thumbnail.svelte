<script lang="ts" module>
  /** The room around the ship in its picture, as a share of the card's width (its p-[8%]). */
  export const THUMBNAIL_INSET = 0.08;
</script>

<script lang="ts">
  // A ship card's picture: our own thumbnail in the current theme
  // (scripts/render-thumbnails.ts, every ship from the same angle on a
  // transparent background), or CCP's studio render for a ship that has none
  // yet, then the card's own fallback. Our thumbnails sit on a glow in the
  // ship's race color, deep in the dark theme and faint in the light one,
  // with its faction's emblem watermarked behind it.
  import type { Snippet } from 'svelte';
  import FactionEmblem from '#lib/components/faction-emblem.svelte';
  import GameImage from '#lib/components/game-image.svelte';
  import { raceTint, typeRender } from '#lib/eve.js';
  import { theme } from '#lib/theme.svelte.js';

  let {
    id,
    raceId,
    logoUrl = null,
    class: className = '',
    fallback,
  }: {
    /** The ship's type id. */
    id: number;
    /** The ship's race, whose color the backdrop takes. */
    raceId: number | null;
    /** The ship's faction emblem, watermarked behind it. */
    logoUrl?: string | null;
    class?: string;
    fallback?: Snippet;
  } = $props();

  /** CCP's render, at the size the cards used before thumbnails. */
  const RENDER_SIZE = 256;
  /** The emblem watermark's strength: present, but well behind the ship. */
  const EMBLEM_OPACITY = 0.12;

  let missing = $state(false);
  // A different ship gets a fresh chance; a theme change only swaps files.
  $effect(() => {
    void id;
    missing = false;
  });
</script>

{#if missing}
  <GameImage src={typeRender(id, RENDER_SIZE)} alt="" class="object-cover {className}" {fallback} />
{:else}
  <div class="backdrop absolute inset-0" style:--race={raceTint(raceId)}>
    {#if logoUrl}
      <FactionEmblem
        url={logoUrl}
        tint={raceTint(raceId)}
        opacity={EMBLEM_OPACITY}
        class="absolute inset-[14%]"
      />
    {/if}
  </div>
  <img
    src="/api/thumbnails/{id}-{theme.current}.png"
    alt=""
    class="relative object-contain p-[8%] {className}"
    data-thumbnail
    onerror={() => (missing = true)}
  />
{/if}

<style>
  .backdrop {
    background: radial-gradient(
      70% 70% at 50% 42%,
      color-mix(in oklab, var(--race) var(--race-backdrop-core), var(--background)),
      color-mix(in oklab, var(--race) var(--race-backdrop-edge), var(--background))
    );
  }
</style>

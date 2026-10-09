<script lang="ts">
  // A livery as the game shows it: the skin material's own icon, the angled
  // hexagon from the market and the SKINs window. A livery without one (a
  // ship's standard issue) gets the same hexagon composed the way the client
  // does it: its greyscale layers tinted with the livery's colors. Done in
  // CSS: each layer's shape is a mask filled with its color, and the grey
  // layer multiplies over it for the shading.
  import type { Colors } from '$lib/types';

  let {
    iconUrl = null,
    colors,
    selected = false,
    class: className = '',
  }: {
    iconUrl?: string | null;
    colors: Colors;
    selected?: boolean;
    class?: string;
  } = $props();

  /** The tint for a color the SDE leaves out. */
  const FALLBACK = '#5a5f66';

  const layerUrl = (name: string) => `/api/skins/layers/${name}.png`;

  /**
   * The client's layers, bottom to top, and the color each is tinted with
   * (none: as is). Its `glow` layer is the hover highlight, so it is left out.
   */
  const layers = $derived([
    { name: 'main', tint: colors.primary ?? FALLBACK },
    { name: 'secondary', tint: colors.secondary ?? colors.primary ?? FALLBACK },
    { name: 'hull', tint: colors.hull ?? FALLBACK },
    { name: 'lights', tint: null },
  ]);
</script>

<span class="skin-icon relative block {className}" class:selected>
  {#if iconUrl}
    <img src={iconUrl} alt="" class="size-full" loading="lazy" />
  {:else}
    {#each layers as layer (layer.name)}
      {#if layer.tint}
        <span
          class="tint absolute inset-0"
          style:background={layer.tint}
          style:--mask="url({layerUrl(layer.name)})"
        ></span>
        <img src={layerUrl(layer.name)} alt="" class="shade absolute inset-0 size-full" />
      {:else}
        <img src={layerUrl(layer.name)} alt="" class="absolute inset-0 size-full" />
      {/if}
    {/each}
  {/if}
</span>

<style>
  .skin-icon {
    isolation: isolate;
    transition:
      transform 0.5s var(--ease-apple),
      filter 0.5s var(--ease-apple);
  }
  .skin-icon.selected {
    transform: scale(1.12);
    filter: drop-shadow(0 0 6px rgb(255 255 255 / 0.55));
  }
  .tint {
    mask: var(--mask) center / 100% 100% no-repeat;
  }
  .shade {
    mix-blend-mode: multiply;
  }
</style>

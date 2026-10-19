<script lang="ts">
  // Each in-game SKIN icon (left) next to our vector redraw (right), to judge
  // the redraw against the original. `?ships=1,2,3` picks the hulls.
  import SkinHexagon from '#lib/components/skin-hexagon.svelte';

  let { data } = $props();

  let size = $state(112);
</script>

<main class="relative z-20 mx-auto max-w-7xl px-6 pt-28 pb-20">
  <div class="mb-8 flex flex-wrap items-center gap-6">
    <h1 class="text-2xl font-semibold tracking-tight">SKIN icons: in-game vs ours</h1>
    <label class="flex items-center gap-3 text-sm text-muted-foreground">
      Size
      <input type="range" min="32" max="320" step="8" bind:value={size} />
      <span class="w-12 tabular-nums">{size} px</span>
    </label>
  </div>

  {#each data.ships as ship (ship.id)}
    <h2 class="mt-12 mb-4 text-lg font-medium">{ship.name}</h2>
    <div
      class="grid gap-x-8 gap-y-6"
      style:grid-template-columns="repeat(auto-fill, minmax({size * 2 + 24}px, 1fr))"
    >
      {#each ship.skins.filter((skin) => skin.icon_url) as skin (skin.id)}
        <figure>
          <div class="flex gap-3">
            <img src={skin.icon_url} alt="" style:width="{size}px" style:height="{size}px" />
            {#if skin.icon_colors}
              <div class="shrink-0" style:width="{size}px" style:height="{size}px">
                <SkinHexagon
                  colors={skin.icon_colors}
                  lights={skin.icon_colors.lights}
                  class="size-full"
                />
              </div>
            {:else}
              <span
                class="grid place-items-center text-xs text-muted-foreground"
                style:width="{size}px">no colors</span
              >
            {/if}
          </div>
          <figcaption class="mt-1.5 truncate text-xs text-muted-foreground">
            {skin.name}
          </figcaption>
        </figure>
      {/each}
    </div>
  {/each}
</main>

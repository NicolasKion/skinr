<script lang="ts">
  // A livery as the game shows it: the angled SKIN hexagon from the market and
  // the SKINs window. It is drawn as vectors (SkinHexagon) whenever its colors
  // are known, so it stays sharp at any size: a ship's standard issue, a
  // player's SKINR design, and in-game skins whose colors the import read off
  // their icon. Only a skin without them falls back to the client's 64 px icon.
  import SkinHexagon from '$lib/components/skin-hexagon.svelte';
  import type { IconPattern } from '$lib/skinr';
  import type { Colors, IconColors } from '$lib/types';

  let {
    iconUrl = null,
    iconColors = null,
    colors,
    lights,
    patterns = [],
    selected = false,
    class: className = '',
  }: {
    /** The client's icon, a 64 px image: the fallback when its colors are unknown. */
    iconUrl?: string | null;
    /** The icon's own colors (read off it by the import); drawn sharp when given. */
    iconColors?: IconColors | null;
    colors: Colors;
    lights?: string;
    patterns?: IconPattern[];
    selected?: boolean;
    class?: string;
  } = $props();

  /** The hexagon's corners on the icon's 64-unit grid, clockwise from the top left. */
  const CORNERS = [
    [16, 5],
    [48, 5],
    [63, 32],
    [48, 59],
    [16, 59],
    [1, 32],
  ];
  /**
   * How far the selection ring sits outside the hexagon, in grid units. Small
   * enough that the ring stays within the few pixels rails leave around an icon.
   */
  const RING_GAP = 3;

  /** The hexagon grown by `gap` on every side: each corner moves out along both its edges' normals. */
  function grow(gap: number) {
    const normal = (a: number[], b: number[]) => {
      const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
      const length = Math.hypot(dx, dy);
      return [dy / length, -dx / length];
    };
    return CORNERS.map((corner, index) => {
      const before = normal(CORNERS[(index + CORNERS.length - 1) % CORNERS.length], corner);
      const after = normal(corner, CORNERS[(index + 1) % CORNERS.length]);
      const scale = gap / (1 + before[0] * after[0] + before[1] * after[1]);
      return `${corner[0] + (before[0] + after[0]) * scale} ${corner[1] + (before[1] + after[1]) * scale}`;
    }).join(' ');
  }
  const RING = grow(RING_GAP);
</script>

<span class="skin-icon relative block {className}" class:selected>
  {#if iconColors}
    <SkinHexagon colors={iconColors} lights={iconColors.lights} class="size-full" />
  {:else if iconUrl}
    <img src={iconUrl} alt="" class="size-full" loading="lazy" />
  {:else}
    <SkinHexagon {colors} {patterns} {lights} class="size-full" />
  {/if}
  <svg viewBox="0 0 64 64" class="selection-hex" aria-hidden="true">
    <polygon points={RING} vector-effect="non-scaling-stroke" />
  </svg>
</span>

<style>
  .selection-hex {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
    pointer-events: none;
    fill: none;
    stroke: color-mix(in oklab, var(--foreground) 90%, transparent);
    stroke-width: 1.5px;
    stroke-linejoin: miter;
    opacity: 0;
    transform: scale(0.9);
    transition:
      opacity 0.4s var(--ease-apple),
      transform 0.5s var(--ease-apple);
  }
  .selected .selection-hex {
    opacity: 1;
    transform: none;
  }
</style>

<script lang="ts" module>
  /** The thumbnail widths the API serves as WebP (src/server/thumbnails.rs). */
  export const WEBP_WIDTHS = [256, 512] as const;

  /** Whose thumbnail: a ship's own livery, one of its skins, or a SKINR design on it. */
  export type Livery =
    | { kind: 'ship'; ship: number }
    | { kind: 'skin'; ship: number; skin: number }
    | { kind: 'design'; ship: number; design: string };

  /** The livery's own thumbnail as WebP at `width`, in `theme` (src/thumbnails.rs names). */
  export function liveryThumbnail(livery: Livery, theme: string, width: number): string {
    const name =
      livery.kind === 'skin'
        ? `skins/${livery.ship}_${livery.skin}`
        : livery.kind === 'design'
          ? `designs/${livery.design}`
          : `${livery.ship}`;
    return `/api/thumbnails/${name}-${theme}-${width}.webp`;
  }
</script>

<script lang="ts">
  // A livery's picture on a transparent background: its own thumbnail (the
  // API draws one of every skin and design, src/thumbnails.rs), the bare
  // hull's while it has none yet, then CCP's type icon.
  import { typeIcon } from '#lib/eve.js';
  import { theme } from '#lib/theme.svelte.js';

  let {
    livery,
    width = WEBP_WIDTHS[0],
    alt = '',
    title,
    class: className = '',
  }: {
    livery: Livery;
    /** The WebP width to fetch, one of WEBP_WIDTHS: the drawn size at 2x. */
    width?: (typeof WEBP_WIDTHS)[number];
    alt?: string;
    title?: string;
    class?: string;
  } = $props();

  /** The pictures to try, best first. */
  const sources = $derived([
    ...(livery.kind === 'ship' ? [] : [liveryThumbnail(livery, theme.current, width)]),
    liveryThumbnail({ kind: 'ship', ship: livery.ship }, theme.current, width),
    typeIcon(livery.ship),
  ]);
  /** How many sources failed so far. */
  let failed = $state(0);
  // A different livery gets a fresh chance; a theme change only swaps files.
  $effect(() => {
    void sources[0];
    failed = 0;
  });
</script>

<img
  src={sources[Math.min(failed, sources.length - 1)]}
  {alt}
  {title}
  loading="lazy"
  decoding="async"
  class="object-contain {className}"
  onerror={() => {
    if (failed < sources.length - 1) failed += 1;
  }}
/>

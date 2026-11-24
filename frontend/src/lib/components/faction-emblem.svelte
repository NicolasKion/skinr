<script lang="ts">
  // A faction's emblem as tinted light: the logo (a single-colour shape on
  // transparency) is used as a mask over a gradient in the page's tint, so it
  // reads as a glow behind the ship rather than a grey stamp. On a light page
  // a pale glow vanishes, so there it turns into a deeper, stronger shade.
  //
  // The logos are large images. One marked `late` (the many behind cards) is
  // fetched only once the first ship is up, so it does not share the line
  // with the ship's own files while the boot screen still covers the page.
  import { stage } from '#lib/stage.svelte.js';

  let {
    url,
    tint,
    opacity = 0.14,
    late = false,
    class: className = '',
  }: { url: string; tint: string; opacity?: number; late?: boolean; class?: string } = $props();
</script>

{#if !late || stage.booted}
  <div
    class="emblem {className}"
    style:--emblem="url('{url}')"
    style:--emblem-tint={tint}
    style:--emblem-opacity={opacity}
    aria-hidden="true"
  ></div>
{/if}

<style>
  .emblem {
    opacity: var(--emblem-opacity);
    background: radial-gradient(
      closest-side,
      color-mix(in oklab, var(--emblem-tint) 90%, white),
      var(--emblem-tint) 55%,
      color-mix(in oklab, var(--emblem-tint) 40%, transparent)
    );
    mask: var(--emblem) center / contain no-repeat;
    -webkit-mask: var(--emblem) center / contain no-repeat;
  }
  /* Race tints are pale: darken toward black instead of lightening, and show
     it more strongly than the dark theme's faint glow. */
  :global(:root.light) .emblem {
    opacity: calc(var(--emblem-opacity) * 2.2);
    background: radial-gradient(
      closest-side,
      color-mix(in oklab, var(--emblem-tint) 75%, black),
      color-mix(in oklab, var(--emblem-tint) 60%, black) 55%,
      color-mix(in oklab, var(--emblem-tint) 40%, transparent)
    );
  }
</style>

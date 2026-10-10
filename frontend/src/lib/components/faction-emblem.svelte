<script lang="ts">
  // A faction's emblem as tinted light: the logo (a single-colour shape on
  // transparency) is used as a mask over a gradient in the page's tint, so it
  // reads as a glow behind the ship rather than a grey stamp. On a light page
  // a pale glow vanishes, so there it turns into a deeper, stronger shade.
  let {
    url,
    tint,
    opacity = 0.14,
    class: className = '',
  }: { url: string; tint: string; opacity?: number; class?: string } = $props();
</script>

<div
  class="emblem {className}"
  style:--emblem="url('{url}')"
  style:--emblem-tint={tint}
  style:--emblem-opacity={opacity}
  aria-hidden="true"
></div>

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

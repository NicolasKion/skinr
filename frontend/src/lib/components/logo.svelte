<script lang="ts">
  // The ship-viewer mark: a hull outline in the current color.
  let { class: className = '' }: { class?: string } = $props();
</script>

<svg viewBox="0 0 32 32" class={className} aria-hidden="true">
  <path
    d="M16 2 28 26 16 21 4 26Z"
    fill="none"
    stroke="currentColor"
    stroke-width="2.5"
    stroke-linejoin="round"
  />
</svg>

<script lang="ts">
  // A ship's light game files as preload links: on their way before any
  // script runs (#lib/server/files-ahead.ts; the page's own fetches then join
  // them, resources.ts). `media` keeps them to screens that show the ship.
  let { urls, media }: { urls: string[]; media?: string } = $props();
</script>

<svelte:head>
  {#each urls as href (href)}
    <link rel="preload" as="fetch" crossorigin="anonymous" fetchpriority="low" {href} {media} />
  {/each}
</svelte:head>

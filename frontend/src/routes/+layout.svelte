<script lang="ts">
  import './layout.css';
  import { onNavigate } from '$app/navigation';
  import SiteBar from '$lib/components/site-bar.svelte';
  import Stage from '$lib/components/stage.svelte';

  let { children } = $props();

  // Every navigation runs as a View Transition where the browser has them:
  // the page cross-fades and elements sharing a view-transition-name (a
  // ship's name on a card and as the hero title) morph from one to the other.
  onNavigate((navigation) => {
    if (!document.startViewTransition || navigation.from?.url.href === navigation.to?.url.href) {
      return;
    }
    // Livery changes only touch the query; the stage animates those itself.
    if (navigation.from?.url.pathname === navigation.to?.url.pathname) return;

    // Explorer and ship page sit side by side: opening a ship shifts the page
    // over to the left, going back shifts it the other way (layout.css).
    const from = navigation.from?.route.id;
    const to = navigation.to?.route.id;
    const root = document.documentElement;
    if (from === '/' && to === '/ships/[id]') root.dataset.nav = 'forward';
    else if (from === '/ships/[id]' && to === '/') root.dataset.nav = 'back';
    else delete root.dataset.nav;

    return new Promise((resolve) => {
      const transition = document.startViewTransition(async () => {
        resolve();
        await navigation.complete;
      });
      transition.finished.finally(() => delete root.dataset.nav);
    });
  });
</script>

<svelte:head>
  <link rel="icon" href="/favicon.svg" />
</svelte:head>

<!-- Everything is laid out in one positioned box, so the stage's in-page
     placements (absolute, page coordinates) are measured from the page top
     and clipped to the page instead of growing it. -->
<div class="relative">
  <div class="pointer-events-none absolute inset-0 z-10 overflow-clip">
    <Stage />
  </div>

  <SiteBar />

  {@render children()}

  <footer class="relative z-20 border-t border-foreground/[0.06] bg-background">
    <p class="mx-auto max-w-[90rem] px-6 py-6 text-[0.7rem] leading-relaxed text-muted-foreground">
      EVE Online and all related assets are the property of CCP hf. This tool is not affiliated with
      or endorsed by CCP. Ships are built live from the game client with CarbonEngineJS.
    </p>
  </footer>
</div>

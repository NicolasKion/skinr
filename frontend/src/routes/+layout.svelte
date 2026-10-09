<script lang="ts">
  import './layout.css';
  import { onNavigate } from '$app/navigation';
  import { page } from '$app/state';
  import Logo from '$lib/components/logo.svelte';
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

  let scrollY = $state(0);
  // Over a ship's hero the bar is clear; once the page scrolls it frosts.
  const clearBar = $derived(page.url.pathname.startsWith('/ships/') && scrollY < 40);
</script>

<svelte:head>
  <link rel="icon" href="/favicon.svg" />
</svelte:head>

<svelte:window bind:scrollY />

<!-- Everything is laid out in one positioned box, so the stage's in-page
     placements (absolute, page coordinates) are measured from the page top
     and clipped to the page instead of growing it. -->
<div class="relative">
  <div class="pointer-events-none absolute inset-0 z-10 overflow-clip">
    <Stage />
  </div>

  <header
    class="fixed inset-x-0 top-0 z-40 border-b transition-[background-color,border-color] duration-500 {clearBar
      ? 'border-transparent bg-transparent'
      : 'border-white/[0.06] bg-background/70 backdrop-blur-xl backdrop-saturate-150'}"
  >
    <nav class="mx-auto flex h-12 max-w-[90rem] items-center justify-between px-6">
      <a href="/" class="flex items-center gap-2 text-foreground/90 hover:text-foreground">
        <Logo class="size-5" />
        <span class="text-sm font-semibold tracking-tight">Ship Viewer</span>
      </a>
      <div class="flex items-center gap-6 text-xs text-foreground/70">
        <a href="/" class="hover:text-foreground">Ships</a>
        <a href="https://github.com/carbonenginejs" class="hover:text-foreground">CarbonEngineJS</a>
      </div>
    </nav>
  </header>

  {@render children()}

  <footer class="relative z-20 border-t border-white/[0.06] bg-background">
    <p class="mx-auto max-w-[90rem] px-6 py-6 text-[0.7rem] leading-relaxed text-muted-foreground">
      EVE Online and all related assets are the property of CCP hf. This tool is not affiliated with
      or endorsed by CCP. Ships are built live from the game client with CarbonEngineJS.
    </p>
  </footer>
</div>

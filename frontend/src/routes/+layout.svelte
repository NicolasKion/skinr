<script lang="ts">
  import './layout.css';
  import { dev } from '$app/env';
  import { onNavigate } from '$app/navigation';
  import { page } from '$app/state';
  import BootScreen from '#lib/components/boot-screen.svelte';
  import ShipLeap from '#lib/components/ship-leap.svelte';
  import SiteBar from '#lib/components/site-bar.svelte';
  import Stage from '#lib/components/stage.svelte';
  import { planPage, type PageStop } from '#lib/page-transitions.js';
  import { stage } from '#lib/stage.svelte.js';

  let { children } = $props();

  /** Where the preview panel shows (its `lg:` classes). */
  const PANEL_SHOWS = '(min-width: 64rem)';

  // The stage's trace goes to the console with the debug tools, or in dev.
  $effect(() => {
    stage.debug = dev || Boolean(page.data.debug);
  });

  // Every navigation runs as a View Transition where the browser has them,
  // moving the page the way the table in page-transitions.ts says, while the
  // site bar, the 3D stage and a panel both pages share stay where they are
  // (layout.css). The stage moves itself to where the new page wants it.
  onNavigate((navigation) => {
    if (navigation.shallow && navigation.type === 'goto') return;
    // A leap to a related ship runs under its own cover (ship-leap.svelte).
    if (!document.startViewTransition || stage.leap) return;

    const stop = (end: typeof navigation.to): PageStop | null =>
      end && {
        routeId: end.route.id,
        pathname: end.url.pathname,
        href: end.url.href,
        ship: end.params?.ship,
      };
    const from = stop(navigation.from);
    const to = stop(navigation.to);
    const plan = planPage(from, to, matchMedia(PANEL_SHOWS).matches);
    if (from?.pathname !== to?.pathname) {
      stage.trace(`navigate ${from?.pathname} → ${to?.pathname}: ${plan?.slide ?? 'in place'}`);
    }
    if (!plan) return;

    const root = document.documentElement;
    root.dataset.nav = plan.slide;
    if (plan.peekStatic) root.dataset.peekStatic = '';
    else delete root.dataset.peekStatic;

    return new Promise((resolve) => {
      const transition = document.startViewTransition(async () => {
        resolve();
        await navigation.complete;
      });
      transition.finished.finally(() => {
        delete root.dataset.nav;
        delete root.dataset.peekStatic;
      });
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
  <!-- A leap's ship rides above its cover (ship-leap.svelte, z-30). -->
  <div
    class="pointer-events-none absolute inset-0 overflow-clip {stage.leap &&
    stage.leap.phase !== 'load'
      ? 'z-[35]'
      : 'z-10'}"
  >
    <Stage />
  </div>

  <SiteBar />
  <BootScreen />
  <ShipLeap />

  {@render children()}

  <footer data-site-footer class="relative z-20 border-t border-foreground/[0.06] bg-background">
    <p class="mx-auto max-w-[90rem] px-6 py-6 text-[0.7rem] leading-relaxed text-muted-foreground">
      EVE Online and all related assets are the property of CCP hf. This tool is not affiliated with
      or endorsed by CCP. Ships are built live from the game client with CarbonEngineJS.
    </p>
  </footer>
</div>

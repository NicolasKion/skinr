<script lang="ts">
  import './layout.css';
  import { onNavigate } from '$app/navigation';
  import BootScreen from '#lib/components/boot-screen.svelte';
  import ShipLeap from '#lib/components/ship-leap.svelte';
  import SiteBar from '#lib/components/site-bar.svelte';
  import Stage from '#lib/components/stage.svelte';
  import { leap } from '#lib/leap.svelte.js';
  import { stage } from '#lib/stage.svelte.js';

  let { children } = $props();

  /**
   * The pages in the order the site bar reads them (Ships, a ship, the
   * Paragon Hub, a designer, your own skins): moving right through them the
   * page slides in from the right, moving left from the left.
   */
  const PAGE_ORDER = [
    '/',
    '/ships/[ship]/[...livery]',
    '/skinr',
    '/skinr/designer/[designer]',
    '/me',
  ];
  /** Where the preview panel shows (its `lg:` classes). */
  const PANEL_SHOWS = '(min-width: 64rem)';
  /** The pages with the preview panel; between two of them it stays put. */
  const WITH_PANEL = new Set(['/', '/skinr', '/skinr/designer/[designer]', '/me']);

  // Every navigation runs as a View Transition where the browser has them:
  // the page slides or cross-fades and elements sharing a view-transition-name
  // (a ship's name on a card and as the hero title) morph from one to the
  // other, while the site bar, the 3D stage and a panel both pages share stay
  // where they are (layout.css).
  onNavigate((navigation) => {
    if (navigation.shallow && navigation.type === 'goto') return;

    if (!document.startViewTransition || navigation.from?.url.href === navigation.to?.url.href) {
      return;
    }
    // A filter or a livery stays on its page (a ship's livery is in its path,
    // under the same ship); the stage animates those itself.
    if (navigation.from?.url.pathname === navigation.to?.url.pathname) return;
    const SHIP_PAGE = '/ships/[ship]/[...livery]';
    if (
      navigation.from?.route.id === SHIP_PAGE &&
      navigation.to?.route.id === SHIP_PAGE &&
      navigation.from.params?.ship === navigation.to.params?.ship
    ) {
      return;
    }
    // A leap to a related ship runs under its own cover (ship-leap.svelte).
    if (leap.phase) return;

    const from = navigation.from?.route.id ?? '';
    const to = navigation.to?.route.id ?? '';
    const root = document.documentElement;
    const [fromAt, toAt] = [PAGE_ORDER.indexOf(from), PAGE_ORDER.indexOf(to)];
    // Explorer and ship page sit side by side: opening a ship shifts the page
    // over to the left in step with the stage sweeping out of its panel, going
    // back the other way. A phone has no panel to sweep from, so its pages
    // slide like any other two areas.
    const sweeps = matchMedia(PANEL_SHOWS).matches;
    if (sweeps && from === '/' && to === '/ships/[ship]/[...livery]') root.dataset.nav = 'forward';
    else if (sweeps && from === '/ships/[ship]/[...livery]' && to === '/') {
      root.dataset.nav = 'back';
    } else if (fromAt !== -1 && toAt !== -1 && fromAt !== toAt) {
      root.dataset.nav = toAt > fromAt ? 'area-forward' : 'area-back';
    } else delete root.dataset.nav;
    if (WITH_PANEL.has(from) && WITH_PANEL.has(to)) root.dataset.peekStatic = '';
    else delete root.dataset.peekStatic;
    // Into or out of a designer's page, their portrait and name move between
    // the hub's chip and the page's header; the rest waits for them to land.
    const designer = '/skinr/designer/[designer]';
    if ((from === '/skinr' && to === designer) || (from === designer && to === '/skinr')) {
      root.dataset.morph = '';
    } else delete root.dataset.morph;

    return new Promise((resolve) => {
      const transition = document.startViewTransition(async () => {
        resolve();
        await navigation.complete;
      });
      transition.finished.finally(() => {
        delete root.dataset.nav;
        delete root.dataset.peekStatic;
        delete root.dataset.morph;
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
    class="pointer-events-none absolute inset-0 overflow-clip {stage.leap?.shown
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

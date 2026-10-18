<script lang="ts">
  import { untrack } from 'svelte';
  import { cubicInOut } from 'svelte/easing';
  import type { TransitionConfig } from 'svelte/transition';
  import { ArrowRight, LoaderCircle } from '@lucide/svelte';
  import CardCarousel from '$lib/components/card-carousel.svelte';
  import ShipThumbnail from '$lib/components/ship-thumbnail.svelte';
  import * as Carousel from '$lib/components/ui/carousel';
  import FactionEmblem from '$lib/components/faction-emblem.svelte';
  import Logo from '$lib/components/logo.svelte';
  import { raceTint, shipBuilders } from '$lib/eve';
  import ShipFilters from '$lib/components/ship-filters.svelte';
  import StageAlert from '$lib/components/stage-alert.svelte';
  import { nav } from '$lib/nav.svelte';
  import { CENTER, dockTarget, stage } from '$lib/stage.svelte';
  import { HERO_POSE } from '$lib/viewer/poses';
  import type { ShipSummary } from '$lib/types';

  let { data } = $props();
  const catalog = $derived(data.catalog);

  /** Ship classes from small to large, the order the in-game ship tree reads. */
  const CLASS_ORDER = [
    'Corvette',
    'Shuttle',
    'Frigate',
    'Navy Frigate',
    'Assault Frigate',
    'Interceptor',
    'Covert Ops',
    'Electronic Attack Ship',
    'Stealth Bomber',
    'Logistics Frigate',
    'Expedition Frigate',
    'Prototype Exploration Ship',
    'Destroyer',
    'Interdictor',
    'Command Destroyer',
    'Tactical Destroyer',
    'Cruiser',
    'Heavy Assault Cruiser',
    'Heavy Interdiction Cruiser',
    'Logistics',
    'Combat Recon Ship',
    'Force Recon Ship',
    'Strategic Cruiser',
    'Flag Cruiser',
    'Combat Battlecruiser',
    'Attack Battlecruiser',
    'Command Ship',
    'Battleship',
    'Black Ops',
    'Marauder',
    'Hauler',
    'Deep Space Transport',
    'Blockade Runner',
    'Mining Barge',
    'Exhumer',
    'Industrial Command Ship',
    'Expedition Command Ship',
    'Freighter',
    'Jump Freighter',
    'Capital Industrial Ship',
    'Carrier',
    'Force Auxiliary',
    'Dreadnought',
    'Lancer Dreadnought',
    'Supercarrier',
    'Command Carrier',
    'Titan',
  ];
  const rank = (name: string) => {
    const index = CLASS_ORDER.indexOf(name);
    return index === -1 ? CLASS_ORDER.length : index;
  };

  /** Ships worth opening on, one picked at random for a first visit. */
  const FEATURED = new Set([
    'Raven',
    'Megathron',
    'Apocalypse',
    'Tempest',
    'Astero',
    'Nightmare',
    'Vindicator',
  ]);

  /** The builder tab picked: a race or a faction's key, or null for all. */
  let builder = $state<string | null>(null);
  let query = $state('');

  const groupNames = $derived(new Map(catalog.groups.map((group) => [group.id, group.name])));
  const builders = $derived(shipBuilders(catalog));

  const filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return catalog.ships.filter(
      (ship) =>
        (builder === null || builders.of(ship).key === builder) &&
        (!needle ||
          ship.name.toLowerCase().includes(needle) ||
          (groupNames.get(ship.group_id) ?? '').toLowerCase().includes(needle)),
    );
  });

  const shelves = $derived.by(() => {
    const byGroup = new Map<number, ShipSummary[]>();
    for (const ship of filtered) {
      const list = byGroup.get(ship.group_id) ?? [];
      list.push(ship);
      byGroup.set(ship.group_id, list);
    }
    return catalog.groups
      .filter((group) => byGroup.has(group.id))
      .sort((a, b) => rank(a.name) - rank(b.name) || a.name.localeCompare(b.name))
      .map((group) => ({ group, ships: byGroup.get(group.id) ?? [] }));
  });

  // The stage docks beside the explorer, opening on a featured ship.
  $effect(() => {
    stage.mode = 'dock';
    stage.placement = CENTER;
    stage.pose = HERO_POSE;
    stage.dim = 0;
    stage.hologram = 0;
    stage.dockName = 'dock';
    stage.dockRadius = 32;
    if (!stage.ship) {
      const candidates = catalog.ships.filter((ship) => FEATURED.has(ship.name));
      const pick = candidates[Math.floor(Math.random() * candidates.length)] ?? catalog.ships[0];
      if (pick) stage.show(pick);
    }
  });
  // Picking another ship slides its hull in beside the last one.
  $effect(() => {
    stage.slideShips = true;
    return () => {
      stage.slideShips = false;
    };
  });

  /**
   * Which side a newly picked ship slides in from: from the right for one
   * further along the shelves than the ship on stage, else from the left.
   */
  function sideOf(ship: ShipSummary): 1 | -1 {
    const order = shelves.flatMap((shelf) => shelf.ships.map((candidate) => candidate.id));
    const current = stage.ship ? order.indexOf(stage.ship.id) : -1;
    return order.indexOf(ship.id) >= current ? 1 : -1;
  }

  /**
   * A card's first click previews its ship in the dock; a click on the ship
   * already previewed opens it. Modified clicks (new tab and so on) are left
   * to the browser.
   */
  function onCardClick(event: MouseEvent, ship: ShipSummary) {
    const plain =
      event.button === 0 && !event.metaKey && !event.ctrlKey && !event.shiftKey && !event.altKey;
    if (plain && stage.ship?.id !== ship.id) {
      event.preventDefault();
      stage.show(ship, ship.dna, sideOf(ship));
      return;
    }
    stage.morphId = ship.id;
  }

  // The builder filter's tabs: all, the races, then the factions that build
  // their own ships.
  const builderTabs = $derived<{ id: string | null; name: string; logo?: string | null }[]>([
    { id: null, name: 'All' },
    ...builders.all.map((entry) => ({ id: entry.key, name: entry.name, logo: entry.logo })),
  ]);

  /** Where the site bar ends, in px from the top: its inset, its height and a gap. */
  const BAR_BOTTOM = 72;

  let filterRow = $state<HTMLElement>();
  let results = $state<HTMLElement>();
  /** Room left between the bar and the first result after filtering from the bar, in px. */
  const RESULTS_GAP = 8;
  let pageFilters = $state<ShipFilters>();
  let dockedFilters = $state<ShipFilters>();
  /** Whether the filter row has scrolled up under the site bar. */
  let scrolledPast = $state(false);
  /** Typing in the bar can shrink the list and scroll the row back; stay docked meanwhile. */
  let barSearchFocused = $state(false);
  const filtersDocked = $derived(scrolledPast || barSearchFocused);

  $effect(() => {
    if (!filterRow) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        scrolledPast = !entry.isIntersecting && entry.boundingClientRect.top < BAR_BOTTOM;
      },
      { rootMargin: `-${BAR_BOTTOM}px 0px 0px 0px` },
    );
    observer.observe(filterRow);
    return () => observer.disconnect();
  });

  $effect(() => {
    nav.center = barFilters;
    return () => nav.clear();
  });
  $effect(() => {
    nav.centerShown = filtersDocked;
  });

  // Filtering from the bar brings the results back to their start, right
  // under it, so the list in view is the list the filter made.
  let filterSeen = false;
  $effect(() => {
    void builder;
    void query;
    if (!filterSeen) {
      filterSeen = true;
      return;
    }
    untrack(() => {
      if (!filtersDocked || !results) return;
      const top = results.getBoundingClientRect().top + window.scrollY - BAR_BOTTOM - RESULTS_GAP;
      if (window.scrollY > top) window.scrollTo({ top });
    });
  });

  // The dock's labels, emblem and tint follow the hull on screen, so they
  // change as it slides in rather than when it is picked.
  const docked = $derived(stage.onScreen ?? stage.ship);
  /** The side the hull on screen came in from; its labels follow it. */
  let side = $state<1 | -1>(1);
  $effect.pre(() => {
    void docked?.id;
    side = untrack(() => stage.direction);
  });

  /** How far the dock's labels and emblem travel as hulls change, in px. */
  const SWAP_DISTANCE = 48;
  /** How long they take, in ms: about the hull's own glide. */
  const SWAP_MS = 1100;
  /**
   * Slides an element in from the side the new hull comes from, or out the
   * way the old one leaves, fading and blurring on the way.
   */
  function swap(_node: Element, { leaving }: { leaving: boolean }): TransitionConfig {
    const offset = SWAP_DISTANCE * (leaving ? -side : side);
    const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
    return {
      duration: still ? 0 : SWAP_MS,
      easing: cubicInOut,
      css: (t, u) =>
        `opacity: ${t}; transform: translateX(${u * offset}px); filter: blur(${u * 6}px)`,
    };
  }
  const dockTint = $derived(raceTint(docked?.race_id));
  const factionLogos = $derived(
    new Map(catalog.factions.map((faction) => [faction.id, faction.logo_url])),
  );
  const dockLogo = $derived(
    docked?.faction_id != null ? (factionLogos.get(docked.faction_id) ?? null) : null,
  );

  function onKeydown(event: KeyboardEvent) {
    if (event.key === '/' && !(document.activeElement instanceof HTMLInputElement)) {
      event.preventDefault();
      (filtersDocked ? dockedFilters : pageFilters)?.focusSearch();
    }
  }
</script>

<svelte:head>
  <title>skinr.space · Try on EVE ship skins</title>
</svelte:head>

<svelte:window onkeydown={onKeydown} />

<!-- Two sections side by side: the browser on the left, and the 3D stage as
     the next page peeking in from the right edge. Opening a ship shifts over
     to it (see the directional view transition in +layout.svelte). -->
<!-- The filters as the site bar shows them once docked. -->
{#snippet barFilters()}
  <ShipFilters
    tabs={builderTabs}
    bind:builder
    bind:query
    compact
    bind:this={dockedFilters}
    onfocuschange={(focused) => (barSearchFocused = focused)}
  />
{/snippet}

<main class="explorer px-6 pt-28 pb-24 lg:pr-[calc(var(--peek)+2rem)] lg:pl-10">
  <section class="mb-14 max-w-4xl">
    <p class="eyebrow mb-5 text-muted-foreground">
      {catalog.ships.length} hulls · SKINR market · SDE {catalog.sde_build ?? 'unknown'}
    </p>
    <h1 class="display text-[clamp(3rem,8vw,6.5rem)]">
      <span class="text-sheen">Try on every skin.<br />Before you buy it.</span>
    </h1>
    <p class="mt-6 max-w-2xl text-lg leading-relaxed text-muted-foreground md:text-xl">
      Every skin in EVE Online and every SKINR design up for sale on the Paragon Hub, on the real
      hull in 3D. Pick a ship to preview it, pick it again to see everything it can wear. Selling a
      design? It shows up here once it is listed.
    </p>
  </section>

  <!-- The filters under the headline; once they scroll up to the site bar
       they dock into it (the bar renders barFilters) and fade out here. -->
  <div
    bind:this={filterRow}
    class="filter-row mb-10"
    class:docked={filtersDocked}
    inert={filtersDocked}
  >
    <ShipFilters tabs={builderTabs} bind:builder bind:query bind:this={pageFilters} />
  </div>

  <!-- At least a screen tall, so a short result list can still scroll the
       filter row up under the bar and keep it docked there. -->
  <div bind:this={results} class="min-h-dvh">
    <div class="min-w-0">
      {#each shelves as shelf (shelf.group.id)}
        <section class="reveal mb-12">
          <CardCarousel label="{shelf.group.name} ships">
            {#snippet heading()}
              <div class="flex items-baseline gap-3">
                <h2 class="text-2xl font-semibold tracking-tight">{shelf.group.name}</h2>
                <span class="text-sm text-muted-foreground">{shelf.ships.length}</span>
              </div>
            {/snippet}
            {#each shelf.ships as ship (ship.id)}
              {@const tint = ship.colors.primary ?? raceTint(ship.race_id)}
              <Carousel.Item class="basis-auto">
                <a
                  href="/ships/{ship.id}"
                  class="group block w-52"
                  onclick={(event) => onCardClick(event, ship)}
                  style:--glow={tint}
                >
                  <div class="relative">
                    <!-- A soft halo in the livery's color, blooming behind the tile on hover. -->
                    <div
                      class="absolute inset-4 rounded-full opacity-0 blur-2xl transition-opacity duration-700 group-hover:opacity-60"
                      style:background="var(--glow)"
                    ></div>
                    <div
                      class="relative aspect-square overflow-hidden rounded-[1.75rem] bg-foreground/[0.03] shadow-[0_30px_60px_-30px_var(--shadow),inset_0_0_0_1px_color-mix(in_oklab,var(--foreground)_4%,transparent)] transition-transform duration-500 ease-[var(--ease-apple)] {stage
                        .ship?.id === ship.id
                        ? 'shadow-[0_30px_60px_-30px_rgb(0_0_0/0.9),inset_0_0_0_1.5px_rgb(255_255_255/0.5)]'
                        : ''}"
                    >
                      <ShipThumbnail
                        id={ship.id}
                        raceId={ship.race_id}
                        logoUrl={ship.faction_id != null ? factionLogos.get(ship.faction_id) : null}
                        class="h-full w-full transition-transform duration-700 ease-[var(--ease-apple)] group-hover:-translate-y-[3%] group-hover:scale-[1.1]"
                      >
                        {#snippet fallback()}
                          <div
                            class="grid h-full w-full place-items-center"
                            style:background="radial-gradient(60% 60% at 50% 45%, color-mix(in
                            oklab, {tint}
                            22%, transparent), transparent)"
                          >
                            <Logo class="size-10 text-foreground/25" />
                          </div>
                        {/snippet}
                      </ShipThumbnail>
                    </div>
                  </div>
                  <p
                    class="mt-3.5 truncate text-[0.95rem] font-medium"
                    style:view-transition-name={stage.morphId === ship.id ? 'ship-name' : undefined}
                  >
                    {ship.name}
                  </p>
                  <p class="truncate text-xs text-muted-foreground">
                    {builders.of(ship).name}
                  </p>
                </a>
              </Carousel.Item>
            {/each}
          </CardCarousel>
        </section>
      {:else}
        <p class="py-20 text-center text-muted-foreground">No ships match “{query}”.</p>
      {/each}
    </div>
  </div>
</main>

<!-- The peeking stage, in two layers around the canvas (z-10): its frame sits
     behind it, its labels in front. Both are fixed, and a fixed element is its
     own stacking context, so they cannot share one element. The frame runs
     past the right edge so only its left corners show. -->
<div
  class="explorer peek peek-frame pointer-events-none fixed top-[4.5rem] right-[-3rem] bottom-3 z-0 hidden rounded-l-[2.25rem] lg:block"
  style:--dock-tint={dockTint}
  use:dockTarget
>
  {#key dockLogo}
    <div
      class="absolute top-1/2 left-[42%] size-[min(48rem,70vh)] -translate-x-1/2 -translate-y-1/2"
      in:swap={{ leaving: false }}
      out:swap={{ leaving: true }}
    >
      {#if dockLogo}
        <FactionEmblem url={dockLogo} tint={dockTint} opacity={0.1} class="size-full" />
      {/if}
    </div>
  {/key}
</div>

{#if docked}
  <div
    class="explorer peek pointer-events-none fixed top-[4.5rem] right-0 bottom-3 z-20 hidden flex-col justify-between p-9 lg:flex"
  >
    <!-- Labels stack in one cell, so the old one can leave while the new one arrives. -->
    <div class="grid">
      {#key docked.id}
        <div class="[grid-area:1/1]" in:swap={{ leaving: false }} out:swap={{ leaving: true }}>
          <StageAlert name={docked.name} class="pointer-events-auto mb-6" />
          <p class="eyebrow" style:color={dockTint}>
            {groupNames.get(docked.group_id)} · {builders.of(docked).name}
          </p>
          <p class="display mt-3 text-[clamp(2.5rem,4vw,4rem)]">{docked.name}</p>
        </div>
      {/key}
    </div>
    <div class="flex items-center justify-between gap-4">
      <span class="flex items-center gap-2 text-xs text-muted-foreground">
        {#if stage.status === 'starting' || stage.status === 'loading'}
          <LoaderCircle class="size-3.5 animate-spin" />
          {stage.status === 'starting' ? 'Starting the engine' : 'Assembling'}
        {:else if stage.status === 'error'}
          Could not assemble
        {:else}
          Drag to rotate
        {/if}
      </span>
      <a
        href="/ships/{docked.id}"
        class="pointer-events-auto flex items-center gap-1.5 rounded-full bg-foreground px-5 py-2.5 text-sm font-medium text-background transition-transform hover:scale-[1.03]"
        onclick={() => (stage.morphId = null)}
      >
        Open <ArrowRight class="size-4" />
      </a>
    </div>
  </div>
{/if}

<style>
  /* The peeking stage's width: a generous third of the screen. */
  .explorer {
    --peek: clamp(24rem, 34vw, 38rem);
  }
  .peek {
    left: calc(100% - var(--peek));
  }
  .peek.right-\[-3rem\] {
    box-shadow:
      inset 1px 0 0 color-mix(in oklab, var(--foreground) 7%, transparent),
      -40px 0 80px -40px var(--shadow);
  }
  /* Registered so the race glow fades from one race's color to the next. */
  @property --dock-tint {
    syntax: '<color>';
    inherits: false;
    initial-value: transparent;
  }
  .peek-frame {
    background:
      radial-gradient(
        70% 55% at 40% 50%,
        color-mix(in oklab, var(--dock-tint) 20%, transparent),
        transparent 70%
      ),
      linear-gradient(
        90deg,
        color-mix(in oklab, var(--foreground) 5%, transparent),
        color-mix(in oklab, var(--foreground) 1.5%, transparent) 40%
      );
    transition: --dock-tint 1.3s var(--ease-apple);
  }
  /* Fades up and out as it docks into the site bar, and back on the way down. */
  .filter-row {
    transition:
      opacity 0.35s var(--ease-apple),
      translate 0.5s var(--ease-apple);
  }
  .filter-row.docked {
    opacity: 0;
    translate: 0 -0.75rem;
  }
</style>

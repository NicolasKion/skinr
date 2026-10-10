<script lang="ts">
  import { untrack } from 'svelte';
  import { ArrowRight, LoaderCircle } from '@lucide/svelte';
  import GameImage from '$lib/components/game-image.svelte';
  import FactionEmblem from '$lib/components/faction-emblem.svelte';
  import Logo from '$lib/components/logo.svelte';
  import { raceTint, typeRender } from '$lib/eve';
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

  let race = $state<number | null>(null);
  let query = $state('');

  const groupNames = $derived(new Map(catalog.groups.map((group) => [group.id, group.name])));
  const raceNames = $derived(new Map(catalog.races.map((entry) => [entry.id, entry.name])));

  const filtered = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    return catalog.ships.filter(
      (ship) =>
        (race === null || ship.race_id === race) &&
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
      stage.show(ship);
      return;
    }
    stage.morphId = ship.id;
  }

  // The race filter's tabs: all, then each race in the catalog.
  const raceTabs = $derived<{ id: number | null; name: string }[]>([
    { id: null, name: 'All' },
    ...catalog.races,
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
    void race;
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

  const docked = $derived(stage.ship);
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
  <title>skinr.space</title>
</svelte:head>

<svelte:window onkeydown={onKeydown} />

<!-- Two sections side by side: the browser on the left, and the 3D stage as
     the next page peeking in from the right edge. Opening a ship shifts over
     to it (see the directional view transition in +layout.svelte). -->
<!-- The filters as the site bar shows them once docked. -->
{#snippet barFilters()}
  <ShipFilters
    races={raceTabs}
    bind:race
    bind:query
    compact
    bind:this={dockedFilters}
    onfocuschange={(focused) => (barSearchFocused = focused)}
  />
{/snippet}

<main class="explorer px-6 pt-28 pb-24 lg:pr-[calc(var(--peek)+2rem)] lg:pl-10">
  <section class="mb-14 max-w-4xl">
    <p class="eyebrow mb-5 text-muted-foreground">
      {catalog.ships.length} ships · SDE {catalog.sde_build ?? 'unknown'}
    </p>
    <h1 class="display text-[clamp(3rem,8vw,6.5rem)]">
      <span class="text-sheen">Every ship.<br />In three dimensions.</span>
    </h1>
    <p class="mt-6 max-w-2xl text-lg leading-relaxed text-muted-foreground md:text-xl">
      Built live from the EVE Online client and drawn with EVE's own shaders, right in your browser.
      Click a ship to preview it, click it again to fly around it.
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
    <ShipFilters races={raceTabs} bind:race bind:query bind:this={pageFilters} />
  </div>

  <!-- At least a screen tall, so a short result list can still scroll the
       filter row up under the bar and keep it docked there. -->
  <div bind:this={results} class="min-h-dvh">
    <div class="min-w-0">
      {#each shelves as shelf (shelf.group.id)}
        <section class="reveal mb-12">
          <div class="mb-4 flex items-baseline gap-3">
            <h2 class="text-2xl font-semibold tracking-tight">{shelf.group.name}</h2>
            <span class="text-sm text-muted-foreground">{shelf.ships.length}</span>
          </div>
          <div
            class="-mx-6 flex snap-x snap-mandatory scroll-px-6 gap-4 overflow-x-auto px-6 pb-4 no-scrollbar"
          >
            {#each shelf.ships as ship (ship.id)}
              {@const tint = ship.colors.primary ?? raceTint(ship.race_id)}
              <a
                href="/ships/{ship.id}"
                class="group w-52 shrink-0 snap-start"
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
                    <GameImage
                      src={typeRender(ship.id, 256)}
                      alt=""
                      class="h-full w-full object-cover transition-transform duration-700 ease-[var(--ease-apple)] group-hover:-translate-y-[3%] group-hover:scale-[1.1]"
                    >
                      {#snippet fallback()}
                        <div
                          class="grid h-full w-full place-items-center"
                          style:background="radial-gradient(60% 60% at 50% 45%, color-mix(in oklab, {tint}
                          22%, transparent), transparent)"
                        >
                          <Logo class="size-10 text-foreground/25" />
                        </div>
                      {/snippet}
                    </GameImage>
                  </div>
                </div>
                <p
                  class="mt-3.5 truncate text-[0.95rem] font-medium"
                  style:view-transition-name={stage.morphId === ship.id ? 'ship-name' : undefined}
                >
                  {ship.name}
                </p>
                <p class="truncate text-xs text-muted-foreground">
                  {raceNames.get(ship.race_id ?? -1) ?? 'Unaffiliated'}
                </p>
              </a>
            {/each}
          </div>
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
  class="explorer peek pointer-events-none fixed top-[4.5rem] right-[-3rem] bottom-3 z-0 hidden rounded-l-[2.25rem] transition-[background] duration-700 lg:block"
  style:background="radial-gradient(70% 55% at 40% 50%, color-mix(in oklab, {dockTint} 20%, transparent),
  transparent 70%), linear-gradient(90deg, color-mix(in oklab, var(--foreground) 5%, transparent), color-mix(in
  oklab, var(--foreground) 1.5%, transparent) 40%)"
  use:dockTarget
>
  {#if dockLogo}
    {#key dockLogo}
      <FactionEmblem
        url={dockLogo}
        tint={dockTint}
        opacity={0.1}
        class="peek-emblem absolute top-1/2 left-[42%] size-[min(48rem,70vh)] -translate-x-1/2 -translate-y-1/2"
      />
    {/key}
  {/if}
</div>

{#if docked}
  <div
    class="explorer peek pointer-events-none fixed top-[4.5rem] right-0 bottom-3 z-20 hidden flex-col justify-between p-9 lg:flex"
  >
    {#key docked.id}
      <div class="peek-label">
        <StageAlert name={docked.name} class="pointer-events-auto mb-6" />
        <p class="eyebrow" style:color={dockTint}>
          {groupNames.get(docked.group_id)} · {raceNames.get(docked.race_id ?? -1) ??
            'Unaffiliated'}
        </p>
        <p class="display mt-3 text-[clamp(2.5rem,4vw,4rem)]">{docked.name}</p>
      </div>
    {/key}
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
  .peek-label {
    animation: label-in 0.7s var(--ease-out-expo) both;
  }
  .peek :global(.peek-emblem) {
    animation: label-in 1.2s var(--ease-out-expo) both;
  }
  @keyframes label-in {
    from {
      opacity: 0;
      transform: translateY(10px);
      filter: blur(4px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .peek-label {
      animation: none;
    }
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

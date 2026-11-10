<script lang="ts" module>
  /**
   * The designer whose chip was clicked last: their portrait, name and count
   * carry view-transition names, so they move into the designer page's
   * header (and back). Only one chip may carry them at a time.
   */
  let morphDesigner = $state<number | null>(null);
</script>

<script lang="ts">
  // The Paragon Hub: every SKINR design on sale, newest, cheapest or most
  // listed first, searchable by design, designer and hull, or one designer's
  // whole line-up (/skinr and /skinr/designer/{name}-{id} both draw this).
  // Any design can be tried on its hull in the preview at the right.
  import { peek } from '#lib/peek.svelte.js';
  import Seo from '#lib/components/seo.svelte';
  import { ChevronLeft, ChevronRight, Search } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import CardCarousel from '#lib/components/card-carousel.svelte';
  import DesignTile from '#lib/components/design-tile.svelte';
  import * as Carousel from '#lib/components/ui/carousel/index.js';
  import SkinrDock from '#lib/components/skinr-dock.svelte';
  import { characterPortrait } from '#lib/eve.js';
  import { nav } from '#lib/nav.svelte.js';
  import { skinrStage } from '#lib/skinr-stage.svelte.js';
  import type { Catalog, HubPage, SkinrDesign } from '#lib/types.js';
  import { designerHref } from '#lib/urls.js';

  let { data }: { data: { hub: HubPage; catalog: Catalog } } = $props();
  const hub = $derived(data.hub);

  const ships = $derived(new Map(data.catalog.ships.map((ship) => [ship.id, ship])));
  const factionLogos = $derived(
    new Map(data.catalog.factions.map((faction) => [faction.id, faction.logo_url])),
  );

  const picked = skinrStage(() => ({
    designs: hub.designs,
    components: hub.components,
    slotMaterials: hub.slot_materials,
    ships,
  }));
  const shipName = (design: SkinrDesign) => ships.get(design.ship_type_id)?.name ?? null;
  const dockLogo = $derived.by(() => {
    const factionId = picked.docked.ship?.faction_id;
    return factionId != null ? (factionLogos.get(factionId) ?? null) : null;
  });

  const plex = new Intl.NumberFormat('en-US');
  const priceNote = (design: SkinrDesign) =>
    [
      design.min_price_plex !== null && `From ${plex.format(design.min_price_plex)} PLEX`,
      `${design.listings} ${design.listings === 1 ? 'listing' : 'listings'}`,
    ]
      .filter(Boolean)
      .join(' · ');

  const SORTS = [
    { id: 'newest', label: 'Newest' },
    { id: 'cheapest', label: 'Cheapest' },
    { id: 'popular', label: 'Most listed' },
  ] as const;
  const sort = $derived(page.url.searchParams.get('sort') ?? 'newest');

  /** This page's URL with some filters changed; a new filter starts at page one. */
  function hrefWith(changes: Record<string, string | null>) {
    const url = new URL(page.url.href);
    url.searchParams.delete('skinr');
    if (!('page' in changes)) url.searchParams.delete('page');
    for (const [name, value] of Object.entries(changes)) {
      if (value === null || value === '') url.searchParams.delete(name);
      else url.searchParams.set(name, value);
    }
    return `${url.pathname}${url.search}`;
  }

  /** How long typing pauses before the search runs, in ms. */
  const SEARCH_DELAY = 300;
  let query = $state(page.url.searchParams.get('q') ?? '');
  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      goto(hrefWith({ q: query.trim() || null }), { replace: true, reset: false });
    }, SEARCH_DELAY);
  }

  const pages = $derived(Math.max(1, Math.ceil(hub.total / hub.page_size)));

  $effect(() => {
    nav.clear();
    return () => {
      clearTimeout(searchTimer);
      nav.clear();
    };
  });
</script>

{#if hub.designer}
  {@const name = hub.designer.name ?? 'A designer'}
  <Seo
    title={`${name}'s SKINRs`}
    description={`Every SKINR design ${name} has on sale on the Paragon Hub (${hub.designer.designs} ${hub.designer.designs === 1 ? 'design' : 'designs'}), each on its real hull in 3D.`}
    image={`/og/designers/${hub.designer.id}.png`}
    path={designerHref(hub.designer)}
    type="profile"
    jsonLd={[
      {
        '@type': 'BreadcrumbList',
        itemListElement: [
          ['Paragon Hub', '/skinr'],
          [name, designerHref(hub.designer)],
        ].map(([crumb, path], index) => ({
          '@type': 'ListItem',
          position: index + 1,
          name: crumb,
          item: `${page.data.origin}${path}`,
        })),
      },
    ]}
  />
{:else}
  <Seo
    title="Paragon Hub: every SKINR for sale"
    description={`All ${plex.format(hub.total)} SKINR designs on sale on EVE Online's Paragon Hub, newest, cheapest or most listed first. Try any of them on its hull in 3D.`}
    image="/og/skinr.png"
    path="/skinr"
  />
{/if}

<div>
  <main
    class="min-h-svh px-6 pt-28 pb-24 lg:pr-[calc(var(--peek)+2rem)] lg:pl-10"
    style:--peek={peek.css}
  >
    {#if hub.designer}
      <header class="mb-14 flex items-center gap-6">
        <img
          src={characterPortrait(hub.designer.id, 256)}
          alt=""
          class="size-24 rounded-full shadow-[0_30px_60px_-30px_var(--shadow)] md:size-28"
          style:view-transition-name="designer-portrait"
        />
        <div>
          <p class="eyebrow mb-3 text-muted-foreground">
            <a href="/skinr" class="hover:text-foreground">Paragon Hub</a> · Designer
          </p>
          <h1 class="display text-[clamp(2.75rem,6vw,5rem)]">
            <span class="text-sheen inline-block" style:view-transition-name="designer-name"
              >{hub.designer.name ?? 'Unknown capsuleer'}</span
            >
          </h1>
          <p class="mt-2 text-muted-foreground" style:view-transition-name="designer-count">
            {hub.designer.designs}
            {hub.designer.designs === 1 ? 'design' : 'designs'} on sale in {hub.designer.listings}
            {hub.designer.listings === 1 ? 'listing' : 'listings'}
          </p>
        </div>
      </header>
    {:else}
      <section class="mb-14 max-w-4xl">
        <p class="eyebrow mb-5 text-muted-foreground">
          Paragon Hub · {plex.format(hub.total)} designs on sale
        </p>
        <h1 class="display text-[clamp(3rem,8vw,6.5rem)]">
          <span class="text-sheen">Every SKINR<br />for sale.</span>
        </h1>
        <p class="mt-6 max-w-2xl text-lg leading-relaxed text-muted-foreground md:text-xl">
          The designs capsuleers have up on the Paragon Hub right now, on the real hull in 3D. Pick
          one to try it on, open it on its ship for a closer look.
        </p>
      </section>

      {#if hub.designers.length}
        <section class="reveal mb-14">
          <CardCarousel label="designers">
            {#snippet heading()}
              <h2 class="text-sm font-medium text-muted-foreground">Busiest designers</h2>
            {/snippet}
            {#each hub.designers as designer (designer.id)}
              {@const morphs = morphDesigner === designer.id}
              <Carousel.Item class="basis-auto">
                <a
                  href={designerHref(designer)}
                  class="designer flex shrink-0 items-center gap-3 rounded-full py-1.5 pr-4 pl-1.5"
                  onclick={() => (morphDesigner = designer.id)}
                >
                  <img
                    src={characterPortrait(designer.id, 64)}
                    alt=""
                    class="size-9 rounded-full"
                    style:view-transition-name={morphs ? 'designer-portrait' : undefined}
                  />
                  <span class="text-left">
                    <span
                      class="block text-sm font-medium"
                      style:view-transition-name={morphs ? 'designer-name' : undefined}
                      >{designer.name ?? 'Unknown'}</span
                    >
                    <span
                      class="block text-xs text-muted-foreground"
                      style:view-transition-name={morphs ? 'designer-count' : undefined}
                    >
                      {designer.designs}
                      {designer.designs === 1 ? 'design' : 'designs'}
                    </span>
                  </span>
                </a>
              </Carousel.Item>
            {/each}
          </CardCarousel>
        </section>
      {/if}
    {/if}

    <div class="mb-8 flex flex-wrap items-center gap-3">
      <label
        class="glass flex h-11 min-w-0 flex-1 items-center gap-2 rounded-full px-4 sm:max-w-sm"
      >
        <Search class="size-4 shrink-0 text-muted-foreground" />
        <input
          type="search"
          bind:value={query}
          oninput={onSearch}
          placeholder="Design, designer or hull"
          aria-label="Search designs"
          class="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
        />
      </label>
      <div class="glass flex h-11 items-center rounded-full p-1" role="group" aria-label="Order">
        {#each SORTS as option (option.id)}
          <a
            href={hrefWith({ sort: option.id === 'newest' ? null : option.id })}
            data-sveltekit-replacestate
            data-sveltekit-noscroll
            class="sort rounded-full px-4 py-1.5 text-sm"
            class:active={sort === option.id}
            aria-current={sort === option.id ? 'true' : undefined}
          >
            {option.label}
          </a>
        {/each}
      </div>
    </div>

    {#if hub.designs.length}
      <div class="grid grid-cols-[minmax(0,1fr)] gap-4 sm:grid-cols-2 2xl:grid-cols-3">
        {#each hub.designs as design (design.id)}
          <DesignTile
            {design}
            components={hub.components}
            shipName={shipName(design)}
            selected={picked.design?.id === design.id}
            onselect={() => picked.pick(design, hub.designs)}
          >
            {priceNote(design)}
          </DesignTile>
        {/each}
      </div>
    {:else}
      <p class="py-20 text-center text-muted-foreground">
        {query ? `No designs match “${query}”.` : 'Nothing on sale here right now.'}
      </p>
    {/if}

    {#if pages > 1}
      <nav class="mt-12 flex items-center justify-center gap-4 text-sm" aria-label="Pages">
        {#if hub.page > 1}
          <a
            href={hrefWith({ page: String(hub.page - 1) })}
            class="pager glass"
            aria-label="Previous page"
          >
            <ChevronLeft class="size-4" />
          </a>
        {/if}
        <span class="text-muted-foreground tabular-nums">{hub.page} / {pages}</span>
        {#if hub.page < pages}
          <a
            href={hrefWith({ page: String(hub.page + 1) })}
            class="pager glass"
            aria-label="Next page"
          >
            <ChevronRight class="size-4" />
          </a>
        {/if}
      </nav>
    {/if}
  </main>

  {#if picked.design}
    <SkinrDock
      design={picked.docked.design}
      ship={picked.docked.ship}
      logoUrl={dockLogo}
      note={picked.docked.design && priceNote(picked.docked.design)}
    />
  {/if}
</div>

<style>
  .designer {
    transition: background 0.3s var(--ease-apple);
  }
  .designer:hover {
    background: color-mix(in oklab, var(--foreground) 6%, transparent);
  }
  .sort {
    color: color-mix(in oklab, var(--foreground) 60%, transparent);
    transition:
      color 0.3s var(--ease-apple),
      background 0.3s var(--ease-apple);
  }
  .sort:hover,
  .sort.active {
    color: var(--foreground);
  }
  .sort.active {
    background: color-mix(in oklab, var(--foreground) 10%, transparent);
  }
  .pager {
    display: grid;
    place-items: center;
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 999px;
  }
</style>

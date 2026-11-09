<script lang="ts">
  // A logged-in character's SKINR: the licenses they own, the designs they
  // made, their Paragon Hub listings, what is offered to them, and their
  // component licenses. Any design in the lists can be tried on its hull in
  // the dock at the right. Logged out, the page asks for a login.
  import { peek } from '#lib/peek.svelte.js';
  import Seo from '#lib/components/seo.svelte';
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import DesignTile from '#lib/components/design-tile.svelte';
  import SkinrDock from '#lib/components/skinr-dock.svelte';
  import { allianceLogo, characterPortrait, corporationLogo, raceTint } from '#lib/eve.js';
  import { nav } from '#lib/nav.svelte.js';
  import { componentRuns, listingAudience, listingState } from '#lib/skinr.js';
  import { skinrStage } from '#lib/skinr-stage.svelte.js';
  import type { SkinrDesign, SkinrListing, SkinrOffer } from '#lib/types.js';

  let { data } = $props();
  const mine = $derived(data.mine);

  const ships = $derived(new Map(data.catalog.ships.map((ship) => [ship.id, ship])));
  const factionLogos = $derived(
    new Map(data.catalog.factions.map((faction) => [faction.id, faction.logo_url])),
  );
  const designs = $derived(new Map((mine?.designs ?? []).map((design) => [design.id, design])));
  const designOf = (id: string) => designs.get(id) ?? null;

  /** The page's lists, each entry with the design it shows. */
  const owned = $derived(
    (mine?.licenses ?? []).flatMap((license) => {
      const design = designOf(license.skinr_id);
      return design ? [{ license, design }] : [];
    }),
  );
  const created = $derived((mine?.created ?? []).map(designOf).filter((design) => design !== null));
  const listings = $derived(
    (mine?.listings ?? []).flatMap((listing) => {
      const design = designOf(listing.skinr_id);
      return design ? [{ listing, design }] : [];
    }),
  );
  const offers = $derived(
    (mine?.offers ?? []).flatMap((offer) => {
      const design = designOf(offer.skinr_id);
      return design ? [{ offer, design }] : [];
    }),
  );
  /** Every design in the order the page lists them, for the dock's slide direction. */
  const order = $derived([
    ...owned.map((entry) => entry.design),
    ...created,
    ...listings.map((entry) => entry.design),
    ...offers.map((entry) => entry.design),
  ]);

  const picked = skinrStage(() => ({
    designs: order,
    components: mine?.components ?? {},
    slotMaterials: mine?.slot_materials ?? {},
    ships,
  }));
  const pick = (design: SkinrDesign) => picked.pick(design, order);
  const shipName = (design: SkinrDesign) => ships.get(design.ship_type_id)?.name ?? null;
  const dockLogo = $derived(
    picked.ship?.faction_id != null ? (factionLogos.get(picked.ship.faction_id) ?? null) : null,
  );

  const plex = new Intl.NumberFormat('en-US');
  const date = new Intl.DateTimeFormat('en-GB', { dateStyle: 'medium', timeZone: 'UTC' });
  const price = (value: number | null) => (value === null ? null : `${plex.format(value)} PLEX`);

  /** What the dock says under the picked design's name. */
  const dockNote = $derived.by(() => {
    const design = picked.design;
    if (!design) return null;
    const license = mine?.licenses.find((candidate) => candidate.skinr_id === design.id);
    if (license) {
      const copies = license.unactivated + (license.activated ? 1 : 0);
      return `You own ${copies} ${copies === 1 ? 'license' : 'licenses'}`;
    }
    return design.min_price_plex !== null
      ? `From ${price(design.min_price_plex)} on the Paragon Hub`
      : null;
  });

  const AUDIENCE_NAMES = {
    character: 'You',
    corporation: 'Your corporation',
    alliance: 'Your alliance',
  };
  const UNAVAILABLE_NAMES: Record<string, string> = {
    licenses: 'licenses',
    components: 'component licenses',
    listings: 'listings',
    offers: 'private offers',
    corporation: 'corporation offers',
    alliance: 'alliance offers',
    affiliation: 'corporation',
  };

  function audienceOf(listing: SkinrListing) {
    const audience = listingAudience(listing.target);
    return audience.kind === 'public' ? 'Public' : `For one ${audience.kind}`;
  }
  const offerNote = (offer: SkinrOffer) =>
    [AUDIENCE_NAMES[offer.audience], offer.seller.name && `from ${offer.seller.name}`]
      .filter(Boolean)
      .join(' · ');

  const loginHref = $derived(`${data.me.login ?? '/eve/login'}?return=%2Fme`);
  const loginFailed = $derived(page.url.searchParams.get('login') === 'failed');

  $effect(() => {
    nav.clear();
    return () => nav.clear();
  });
</script>

<Seo
  title={mine ? `${mine.character.name}'s SKINRs` : 'Your SKINRs'}
  description="The SKINR designs you own, made and listed, each on its hull in 3D."
  noindex
/>

{#if mine}
  <div>
    <main
      class="min-h-dvh px-6 pt-28 pb-24 lg:pr-[calc(var(--peek)+2rem)] lg:pl-10"
      style:--peek={peek.css}
    >
      <header class="mb-16 flex flex-wrap items-end justify-between gap-6">
        <div class="flex items-center gap-6">
          <img
            src={characterPortrait(mine.character.id, 256)}
            alt=""
            class="size-24 rounded-full shadow-[0_30px_60px_-30px_var(--shadow)] md:size-28"
          />
          <div>
            <p class="eyebrow mb-3 flex items-center gap-2 text-muted-foreground">
              {#if mine.character.corporation_id}
                <img
                  src={corporationLogo(mine.character.corporation_id, 32)}
                  alt=""
                  class="size-4 rounded"
                />
              {/if}
              {#if mine.character.alliance_id}
                <img
                  src={allianceLogo(mine.character.alliance_id, 32)}
                  alt=""
                  class="size-4 rounded"
                />
              {/if}
              Your SKINRs
            </p>
            <h1 class="display text-[clamp(2.75rem,6vw,5rem)]">
              <span class="text-sheen">{mine.character.name}</span>
            </h1>
          </div>
        </div>
        <form method="post" action="/eve/logout">
          <button
            type="submit"
            class="rounded-full px-4 py-2 text-sm text-muted-foreground transition-colors hover:bg-foreground/[0.06] hover:text-foreground"
          >
            Log out
          </button>
        </form>
      </header>

      {#if mine.unavailable.length}
        <p class="reveal mb-12 max-w-2xl text-sm text-muted-foreground">
          ESI could not tell us your {mine.unavailable
            .map((part) => UNAVAILABLE_NAMES[part] ?? part)
            .join(', ')} right now. The rest is below; try again in a few minutes.
        </p>
      {/if}

      {#if !order.length && !mine.component_licenses.length}
        <section class="reveal max-w-2xl py-10">
          <h2 class="display mb-4 text-[clamp(2rem,3.5vw,3rem)]">
            <span class="text-sheen">Nothing here yet.</span>
          </h2>
          <p class="text-lg leading-relaxed text-muted-foreground">
            Once you own a SKINR, list one or get an offer on the Paragon Hub, it shows up here.
            Meanwhile, see what other capsuleers are selling.
          </p>
          <a
            href="/skinr"
            class="mt-8 inline-block rounded-full bg-foreground px-5 py-2.5 text-sm font-medium text-background transition-transform hover:scale-[1.03]"
          >
            Browse the Paragon Hub
          </a>
        </section>
      {/if}

      {#snippet grid(title: string, note: string, count: number, tiles: Snippet)}
        <section class="reveal mb-16">
          <div class="mb-5 flex items-baseline gap-3">
            <h2 class="text-2xl font-semibold tracking-tight">{title}</h2>
            <span class="text-sm text-muted-foreground tabular-nums">{count}</span>
          </div>
          <p class="mb-6 max-w-xl text-sm text-muted-foreground">{note}</p>
          <div class="grid grid-cols-[minmax(0,1fr)] gap-4 sm:grid-cols-2 2xl:grid-cols-3">
            {@render tiles()}
          </div>
        </section>
      {/snippet}

      {#if owned.length}
        {#snippet ownedTiles()}
          {#each owned as { license, design } (license.skinr_id)}
            <DesignTile
              {design}
              components={mine.components}
              shipName={shipName(design)}
              selected={picked.design?.id === design.id}
              onselect={() => pick(design)}
            >
              {#if license.activated}<span class="text-foreground">Applied</span>{/if}
              {#if license.activated && license.unactivated}<span class="mx-1 text-foreground/25"
                  >·</span
                >{/if}
              {#if license.unactivated}{license.unactivated} in your hangar{/if}
            </DesignTile>
          {/each}
        {/snippet}
        {@render grid(
          'In your collection',
          'The SKINR licenses you own, applied or waiting in your hangar.',
          owned.length,
          ownedTiles,
        )}
      {/if}

      {#if created.length}
        {#snippet createdTiles()}
          {#each created as design (design.id)}
            <DesignTile
              {design}
              components={mine.components}
              shipName={shipName(design)}
              selected={picked.design?.id === design.id}
              onselect={() => pick(design)}
            >
              {#if design.listings}
                {design.listings} on sale{#if design.min_price_plex !== null}, from {price(
                    design.min_price_plex,
                  )}{/if}
              {:else}
                Not on sale
              {/if}
            </DesignTile>
          {/each}
        {/snippet}
        {@render grid(
          'Made by you',
          'Your designs, as far as we have seen them: ESI has no list of them, so this is what was listed, owned or offered.',
          created.length,
          createdTiles,
        )}
      {/if}

      {#if listings.length}
        {#snippet listingTiles()}
          {#each listings as { listing, design } (listing.id)}
            <DesignTile
              {design}
              components={mine.components}
              shipName={shipName(design)}
              selected={picked.design?.id === design.id}
              onselect={() => pick(design)}
            >
              <span class:text-foreground={listing.state === 'listed'}
                >{listingState(listing.state)}</span
              >
              · {listing.quantity} left{#if listing.price_plex !== null}
                · {price(listing.price_plex)}{/if}
              · {audienceOf(listing)}
              {#if listing.state === 'listed'}· until {date.format(new Date(listing.expires))}{/if}
            </DesignTile>
          {/each}
        {/snippet}
        {@render grid(
          'Your listings',
          'What you put up on the Paragon Hub, live or past.',
          listings.length,
          listingTiles,
        )}
      {/if}

      {#if offers.length}
        {#snippet offerTiles()}
          {#each offers as { offer, design } (offer.id)}
            <DesignTile
              {design}
              components={mine.components}
              shipName={shipName(design)}
              selected={picked.design?.id === design.id}
              onselect={() => pick(design)}
            >
              {offerNote(offer)}{#if offer.price_plex !== null}
                · {price(offer.price_plex)}{/if}
            </DesignTile>
          {/each}
        {/snippet}
        {@render grid(
          'Offered to you',
          'Listings only you, your corporation or your alliance can buy.',
          offers.length,
          offerTiles,
        )}
      {/if}

      {#if mine.component_licenses.length}
        <section class="reveal mb-16">
          <div class="mb-5 flex items-baseline gap-3">
            <h2 class="text-2xl font-semibold tracking-tight">Components</h2>
            <span class="text-sm text-muted-foreground tabular-nums"
              >{mine.component_licenses.length}</span
            >
          </div>
          <p class="mb-6 max-w-xl text-sm text-muted-foreground">
            The nanocoatings and patterns you can sequence new designs with.
          </p>
          <ul class="grid gap-1 sm:grid-cols-2 2xl:grid-cols-3">
            {#each mine.component_licenses as license (`${license.type}-${license.component_id}`)}
              {@const component = mine.components[license.component_id]}
              <li class="flex items-center gap-4 rounded-[1.25rem] p-3">
                {#if component?.icon_url}
                  <img src={component.icon_url} alt="" class="size-12 shrink-0" loading="lazy" />
                {:else}
                  <span
                    class="size-12 shrink-0 rounded-full"
                    style:background={component?.color ?? 'var(--muted)'}
                  ></span>
                {/if}
                <span class="min-w-0">
                  <span class="block truncate font-medium"
                    >{component?.name ?? `Component ${license.component_id}`}</span
                  >
                  <span class="block truncate text-xs text-muted-foreground">
                    {[component?.finish, license.type === 'pattern' ? 'Pattern' : 'Nanocoating']
                      .filter(Boolean)
                      .join(' ')} · {componentRuns(license.runs)}
                  </span>
                </span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </main>

    {#if picked.design}
      <SkinrDock design={picked.design} ship={picked.ship} logoUrl={dockLogo} note={dockNote} />
    {/if}
  </div>
{:else}
  <main
    class="relative z-20 mx-auto flex min-h-dvh max-w-3xl flex-col items-center justify-center px-6 py-28 text-center"
    style:--tint={raceTint(null)}
  >
    <p class="eyebrow mb-5 text-muted-foreground">Your SKINRs</p>
    <h1 class="display text-[clamp(3rem,8vw,6rem)]">
      <span class="text-sheen">Your collection.<br />On the hull.</span>
    </h1>
    <p class="mt-6 max-w-xl text-lg leading-relaxed text-muted-foreground">
      Log in with EVE to see the SKINRs you own, the designs you made, your Paragon Hub listings and
      the offers made to you, each one on its ship in 3D.
    </p>
    {#if data.problem}
      <p class="mt-6 text-sm text-destructive">{data.problem}</p>
    {:else if loginFailed}
      <p class="mt-6 text-sm text-destructive">That login did not go through. Try again?</p>
    {/if}
    {#if data.me.login}
      <a
        href={loginHref}
        data-sveltekit-reload
        class="mt-10 rounded-full bg-foreground px-6 py-3 font-medium text-background transition-transform hover:scale-[1.03]"
      >
        Log in with EVE
      </a>
      <p class="mt-4 max-w-md text-xs text-muted-foreground">
        We only ask to read your cosmetics. Your login stays on our server and you can log out, or
        revoke access on CCP's site, at any time.
      </p>
    {:else}
      <p class="mt-10 text-sm text-muted-foreground">Logging in is not set up on this server.</p>
    {/if}
  </main>
{/if}

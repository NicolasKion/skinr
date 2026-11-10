<script lang="ts">
  // A ship's page, read as chapters: the hero, its profile, the blueprint,
  // the liveries and related ships. This file wires them together: which
  // chapter is on screen decides where the stage sits (`useScene`); each
  // chapter is its own component beside this one.
  import Blueprint from '#lib/components/blueprint.svelte';
  import Seo from '#lib/components/seo.svelte';
  import SheetMargin from '#lib/components/sheet-margin.svelte';
  import { raceTint, shipBuilders } from '#lib/eve.js';
  import { nav } from '#lib/nav.svelte.js';
  import { stage, useScene } from '#lib/stage.svelte.js';
  import type { ShipSummary } from '#lib/types.js';
  import { viewport } from '#lib/viewport.svelte.js';
  import Backdrop from './backdrop.svelte';
  import { CHAPTERS, SECTION_OF, isDocked, whenCrossing, type Chapter } from './chapters';
  import Hero from './hero.svelte';
  import Liveries from './liveries.svelte';
  import { useLivery } from './livery.svelte';
  import { narrowPlacement, textCover } from './narrow';
  import Profile from './profile.svelte';
  import { relatedRails, summaryOf } from './related';
  import Related from './related.svelte';
  import { shipSeo } from './seo';

  let { data } = $props();
  const ship = $derived(data.ship);
  const skinrs = $derived(data.skinrs);
  const tint = $derived(raceTint(ship.race?.id));
  const builders = $derived(shipBuilders(data.catalog));
  const builderOf = (other: ShipSummary) => builders.of(other).name;

  const livery = useLivery(() => ({ ship, skinrs }));
  const seo = $derived(shipSeo(ship, livery.skin, livery.design));
  /** Whether the ship has anything to wear besides its own colors. */
  const wearable = $derived(ship.skins.length > 0 || skinrs.designs.length > 0);
  const rails = $derived(relatedRails(ship, data.catalog.ships));

  // The chapter on screen frames the ship. The browser's size is known from
  // the start (#lib/viewport.svelte.ts), so a phone never shows the wide
  // placement for a frame before switching (the server renders the wide one).
  let chapter = $state<Chapter>('hero');
  const enter = (name: Chapter) => () => (chapter = name);
  const wide = $derived(viewport.wide);
  const framing = $derived(CHAPTERS[chapter]);
  const docked = $derived(isDocked(chapter));
  const placement = $derived(wide ? framing.placement : narrowPlacement(viewport));

  /**
   * On a narrow screen the text scrolls up over the ship above it: the ship
   * fades out while the hero's or the profile's text crosses it.
   */
  let heroText = $state<HTMLElement>();
  let profileText = $state<HTMLElement>();
  let cover = $state(0);
  function measureCover() {
    cover = wide
      ? 0
      : textCover(
          viewport,
          [heroText, profileText].map((text) => text?.getBoundingClientRect() ?? null),
        );
  }

  /**
   * The drawing on the sheet: this ship in the selected livery once drafted.
   * While a new livery is being drafted the previous livery's drawing stays
   * up, so the sheet keeps its size and the page does not jump.
   */
  const drawing = $derived.by(() => {
    const blueprint = stage.blueprint;
    if (!blueprint) return null;
    const sameShip = blueprint.dna.split(':')[0] === livery.dna.split(':')[0];
    return blueprint.dna === livery.dna || sameShip ? blueprint.drawing : null;
  });

  // The chapter places the stage: the hero beside its text, then docked in
  // the blueprint's side elevation from there down. The blueprint is drafted
  // while the reader is still above it.
  useScene(() => ({
    place: docked ? 'blueprint' : 'hero',
    placement,
    pose: framing.pose,
    hologram: docked ? 1 : 0,
    dim:
      !wide && (chapter === 'hero' || chapter === 'profile')
        ? Math.max(framing.dim, cover)
        : framing.dim,
    fit: docked && drawing ? { halfLength: drawing.halfLength } : null,
    draft: true,
  }));

  /**
   * Where the blueprint section sits in the page. Its paper is painted
   * behind the stage (the section itself is above it, with the page's text),
   * so the docked ship shows on it.
   */
  let paper = $state<{ top: number; height: number } | null>(null);
  function paperOf(node: HTMLElement) {
    const measure = () => {
      const rect = node.getBoundingClientRect();
      paper = { top: rect.top + window.scrollY, height: rect.height };
    };
    measure();
    // Its place moves with anything above it, so watch the whole page too.
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    observer.observe(document.body);
    return { destroy: () => observer.disconnect() };
  }

  /**
   * Arrow keys step through a carousel, wrapping around: in the livery
   * chapter, the rail of the look on the ship (the player designs while one
   * is worn), the liveries everywhere else.
   */
  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement)
      return;
    event.preventDefault();
    livery.step(event.key === 'ArrowRight' ? 1 : -1, chapter === 'liveries');
  }

  // The site bar names this ship, links its sections and resets its view.
  $effect(() => {
    nav.title = ship.name;
    nav.sections = [
      ...(ship.description ? [{ id: 'profile', label: 'Profile' }] : []),
      { id: 'blueprint', label: 'Blueprint' },
      ...(wearable ? [{ id: 'liveries', label: 'Skins' }] : []),
      ...(rails.length ? [{ id: 'related', label: 'Related' }] : []),
    ];
    nav.reset = () => stage.resetView?.();
  });
  $effect(() => {
    nav.active = SECTION_OF[chapter];
  });
  $effect(() => () => nav.clear());
</script>

<Seo
  title={seo.title}
  description={seo.description}
  image={seo.image}
  jsonLd={[
    {
      '@type': 'BreadcrumbList',
      itemListElement: seo.crumbs.map(([name, path], index) => ({
        '@type': 'ListItem',
        position: index + 1,
        name,
        item: `${data.origin}${path}`,
      })),
    },
  ]}
/>

<svelte:window onkeydown={onKeydown} onscroll={measureCover} onresize={measureCover} />

<Backdrop {tint} logoUrl={ship.faction?.logo_url ?? null} {placement} {docked} />

<main class="overflow-x-clip">
  {#if paper}
    <div
      class="blueprint-paper pointer-events-none absolute inset-x-0 z-0"
      style:top="{paper.top}px"
      style:height="{paper.height}px"
    >
      <SheetMargin edge="top" />
      <SheetMargin edge="bottom" />
    </div>
  {/if}

  <Hero
    {ship}
    {tint}
    builder={builderOf(summaryOf(ship))}
    {livery}
    card={seo.image}
    onenter={enter('hero')}
    bind:text={heroText}
  />

  <!-- Below the fold every chapter keeps the ship in view: its text takes one
       half of the screen, the stage moves to the other. -->
  <div class="relative z-20" style:--tint={tint}>
    {#if ship.description}
      <Profile {ship} {tint} onenter={enter('profile')} bind:text={profileText} />
    {/if}

    <!-- The blueprint is a sheet of its own: solid blueprint paper from edge to
         edge, set hard against the chapters around it, and dark in either theme. -->
    <section
      id="blueprint"
      class="theme-dark min-h-svh px-4 py-24 lg:px-10"
      use:whenCrossing={enter('blueprint')}
      use:paperOf
    >
      <div class="mx-auto max-w-[90rem]">
        <p class="reveal eyebrow mb-4 text-[var(--blueprint-ink)]">Blueprint</p>
        <h2 class="reveal display mb-10 text-[clamp(2.25rem,4vw,3.5rem)] text-white">
          Everything it takes to build one.
        </h2>
        <Blueprint {ship} {drawing} build={data.catalog.sde_build} built={docked} />
      </div>
    </section>

    {#if wearable}
      <Liveries
        {ship}
        {skinrs}
        {tint}
        {wide}
        {livery}
        card={seo.image}
        onenter={enter('liveries')}
      />
    {/if}

    {#if rails.length}
      <Related {rails} catalog={data.catalog} {builderOf} {tint} onenter={enter('more')} />
    {/if}
  </div>
</main>

<style>
  .blueprint-paper {
    background: var(--blueprint-paper);
  }
  /* The paper lights up the dark chapters next to it, like a lit table. */
  .blueprint-paper::before,
  .blueprint-paper::after {
    content: '';
    position: absolute;
    inset-inline: 0;
    height: 9rem;
    pointer-events: none;
  }
  .blueprint-paper::before {
    bottom: 100%;
    background: linear-gradient(
      to top,
      color-mix(in oklab, var(--blueprint-paper) 45%, transparent),
      transparent
    );
  }
  .blueprint-paper::after {
    top: 100%;
    background: linear-gradient(
      to bottom,
      color-mix(in oklab, var(--blueprint-paper) 45%, transparent),
      transparent
    );
  }
</style>

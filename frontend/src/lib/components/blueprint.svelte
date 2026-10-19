<script lang="ts">
  // The construction sheet, drawn straight onto the fitting window's grid: no
  // frames or panels, just views, dimensions and numbered sections set apart
  // by space. The side elevation is the live ship: the stage docks into the
  // slot marked here (use:dockTarget) and keeps rendering as a hologram. The
  // plan below it and the front beside it are captured drawings at the same
  // scale, centred on the same axis. Once `built` (the ship is on its way in)
  // the rest of the sheet constructs itself around it. Section 05 names the
  // faction that builds the ship; it stays out of the drawing so a slim hull's
  // front view keeps its own width.
  import BonusList from '#lib/components/bonus-list.svelte';
  import DrawnZone from '#lib/components/drawn-zone.svelte';
  import DefenseTable from '#lib/components/defense-table.svelte';
  import FittingSheet from '#lib/components/fitting-sheet.svelte';
  import SystemsSheet from '#lib/components/systems-sheet.svelte';
  import { dockTarget } from '#lib/stage.svelte.js';
  import { TRANSITION_MS } from '#lib/viewer/poses.js';
  import type { Blueprint } from '#lib/viewer/renderer.js';
  import type { ShipDetail } from '#lib/types.js';

  let {
    ship,
    drawing,
    build,
    built,
  }: {
    ship: ShipDetail;
    drawing: Blueprint | null;
    build: string | null;
    /** Whether the ship is docking and the sheet should construct itself. */
    built: boolean;
  } = $props();

  /** Space between views, in CSS pixels. */
  const GAP = 56;
  /** The tallest the drawing may get, as a share of the viewport height. */
  const MAX_DRAWING_HEIGHT = 0.7;
  /** A CSS pixel on paper, in metres (96 per inch). */
  const CSS_PIXEL_METRES = 0.0254 / 96;
  /** The sheet starts constructing once the ship is most of the way into its slot. */
  const BUILD_START = TRANSITION_MS * 0.6;
  /** How far apart the zones start drawing their frames, in ms. */
  const ZONE_STAGGER = 180;

  const metres = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 });

  let areaWidth = $state(0);
  let innerHeight = $state(900);

  const side = $derived(drawing?.views.find((view) => view.name === 'side') ?? null);
  const top = $derived(drawing?.views.find((view) => view.name === 'top') ?? null);
  const front = $derived(drawing?.views.find((view) => view.name === 'front') ?? null);

  /** CSS pixels per metre, shared by every view. */
  const k = $derived.by(() => {
    if (!drawing || !areaWidth) return 0;
    const mpp = drawing.metersPerPixel;
    const length = drawing.halfLength * 2;
    const frontWidth = (front?.width ?? 0) * mpp;
    const height = drawing.halfHeight * 2 + (top?.height ?? 0) * mpp;
    if (!length || !height) return 0;
    const room = areaWidth - GAP * 2;
    return Math.min(
      room / (length + frontWidth),
      (innerHeight * MAX_DRAWING_HEIGHT - GAP) / height,
    );
  });
  /** CSS pixels per captured pixel. */
  const px = $derived(drawing ? k * drawing.metersPerPixel : 0);

  const slot = $derived(
    drawing ? { width: drawing.halfLength * 2 * k, height: drawing.halfHeight * 2 * k } : null,
  );

  /** Where the hull ends in the side elevation's slot, in CSS pixels. */
  const hullBottom = $derived(
    slot && side ? slot.height / 2 + (side.height - side.centerY) * px : (slot?.height ?? 0),
  );

  /** The drawing's scale on a screen, rounded to two figures like a real sheet's. */
  const scale = $derived.by(() => {
    if (!k) return null;
    const ratio = 1 / (k * CSS_PIXEL_METRES);
    const step = 10 ** (Math.floor(Math.log10(ratio)) - 1);
    return `1:${metres.format(Math.round(ratio / step) * step)}`;
  });

  const figure = (metresValue: number | undefined) =>
    metresValue ? `${metres.format(metresValue)} m` : '-';

  const titleBlock = $derived([
    ['Vessel', ship.name],
    ['Class', ship.group.name],
    ['Builder', ship.faction?.name ?? ship.race?.name ?? '-'],
    ['Tech', ship.meta_group?.name ?? '-'],
    ['Length', figure(drawing?.length)],
    ['Drawing', `DWG-${ship.id}`],
    ['SDE', build ?? '-'],
  ]);
</script>

<svelte:window bind:innerHeight />

{#snippet heading(number: string, title: string, note: string)}
  <header class="mb-7">
    <div class="flex items-baseline gap-4">
      <span class="section-number">{number}</span>
      <h3 class="section-title">{title}</h3>
    </div>
    <p class="section-note">{note}</p>
  </header>
{/snippet}

<div class="sheet grid lg:grid-cols-2" class:built style:--start="{BUILD_START}ms">
  <!-- The drawing: the only zone on drafting grid. -->
  <DrawnZone {built} class="drawing-zone lg:col-span-2">
    <p class="build strip" style:--delay="0ms">
      <span>DWG-{ship.id}</span>
      <span>Scale {scale ?? '-'}</span>
      <span>Sheet 1 of 1</span>
    </p>
    <div class="px-6 pt-4 pb-10" bind:clientWidth={areaWidth}>
      {#if drawing && slot && k}
        <div class="flex items-start justify-center" style:gap="{GAP}px">
          <div class="flex flex-col" style:gap="{GAP * 0.8}px">
            <!-- The side elevation: the live ship docks here. -->
            <figure class="relative" style:width="{slot.width}px">
              <!-- Ends at the hull's lowest point; the slot below it is empty. -->
              <div class="relative" style:height="{hullBottom}px">
                <div
                  class="absolute inset-x-0 top-0"
                  style:height="{slot.height}px"
                  use:dockTarget={'blueprint'}
                ></div>
                <!-- The hull's own box inside the slot, for its dimension lines. -->
                {#if side}
                  <div
                    class="pointer-events-none absolute"
                    style:left="{slot.width / 2 - side.centerX * px}px"
                    style:top="{slot.height / 2 - side.centerY * px}px"
                    style:width="{side.width * px}px"
                    style:height="{side.height * px}px"
                  >
                    <div class="dimension horizontal build" style:--delay="400ms">
                      <span>{figure(drawing.length)}</span>
                    </div>
                    <div class="dimension vertical build" style:--delay="500ms">
                      <span>{figure(drawing.height)}</span>
                    </div>
                  </div>
                {/if}
              </div>
              <figcaption class="build" style:--delay="300ms"><b>A</b> Side elevation</figcaption>
            </figure>

            {#if top}
              <figure class="build relative" style:--delay="150ms" style:width="{slot.width}px">
                <div
                  class="relative w-fit"
                  style:margin-left="{slot.width / 2 - top.centerX * px}px"
                >
                  <img src={top.url} alt="{ship.name}, plan" style:width="{top.width * px}px" />
                  <div class="dimension vertical build" style:--delay="600ms">
                    <span>{figure(drawing.beam)}</span>
                  </div>
                </div>
                <figcaption><b>B</b> Plan</figcaption>
              </figure>
            {/if}
          </div>

          <div>
            {#if front}
              <figure
                class="build relative"
                style:--delay="250ms"
                style:margin-top="{slot.height / 2 - front.centerY * px}px"
              >
                <div class="relative w-fit">
                  <img
                    src={front.url}
                    alt="{ship.name}, front"
                    style:width="{front.width * px}px"
                  />
                  <div class="dimension horizontal build" style:--delay="700ms">
                    <span>{figure(drawing.beam)}</span>
                  </div>
                </div>
                <figcaption><b>C</b> Front</figcaption>
              </figure>
            {/if}
          </div>
        </div>
      {:else}
        <div class="grid h-[40vh] place-items-center">
          <p class="font-mono text-sm tracking-widest text-[#8fc8ff]/70 uppercase">
            <span class="drafting">Drafting {ship.name}</span>
          </p>
        </div>
      {/if}
    </div>
  </DrawnZone>

  <!-- The numbered sections on plain paper, each zone drawing its frame in turn. -->
  <DrawnZone {built} delay={ZONE_STAGGER} class="zone-pad">
    {@render heading('01', 'Fitting', 'Slots, hardpoints and the resources a fit spends')}
    <FittingSheet sheet={ship.sheet} />
  </DrawnZone>
  <DrawnZone {built} delay={ZONE_STAGGER * 2} class="zone-pad">
    {@render heading('02', 'Defense', 'Hit points and resistances per layer')}
    <DefenseTable sheet={ship.sheet} />
  </DrawnZone>
  <DrawnZone {built} delay={ZONE_STAGGER * 3} class="zone-pad">
    {@render heading('03', 'Systems', 'Capacitor, targeting and navigation')}
    <SystemsSheet sheet={ship.sheet} />
  </DrawnZone>
  <DrawnZone {built} delay={ZONE_STAGGER * 4} class="zone-pad">
    {@render heading('04', 'Bonuses', 'Per skill level, then role')}
    <BonusList groups={ship.bonuses} />
  </DrawnZone>

  {#if ship.faction}
    <!-- The builder's seal: its emblem in ink, its motto and story. -->
    <DrawnZone {built} delay={ZONE_STAGGER * 5} class="zone-pad lg:col-span-2">
      {@render heading('05', 'Issuing authority', 'The faction that builds the ship')}
      <div class="flex flex-col gap-8 sm:flex-row sm:items-start">
        {#if ship.faction.logo_url}
          <div class="seal">
            <div class="emblem" style:--emblem="url('{ship.faction.logo_url}')"></div>
          </div>
        {/if}
        <div class="max-w-3xl">
          <p class="faction-name">{ship.faction.name}</p>
          {#if ship.faction.tagline}
            <p class="tagline">{ship.faction.tagline}</p>
          {/if}
          {#if ship.faction.description}
            <p class="story">{ship.faction.description}</p>
          {/if}
        </div>
      </div>
    </DrawnZone>
  {/if}

  <!-- The title block. -->
  <DrawnZone {built} delay={ZONE_STAGGER * 6} class="lg:col-span-2">
    <dl class="title-block">
      {#each titleBlock as [label, value] (label)}
        <div>
          <dt class="title-label">{label}</dt>
          <dd class="mt-1 font-mono text-sm text-white">{value}</dd>
        </div>
      {/each}
    </dl>
  </DrawnZone>
</div>

<style>
  /* Everything but the live ship builds in at its own --delay, after the
     ship has mostly arrived (--start). */
  .build {
    opacity: 0;
    transform: translateY(10px);
    filter: blur(4px);
    transition:
      opacity 0.7s var(--ease-out-expo) calc(var(--start) + var(--delay, 0ms)),
      transform 0.9s var(--ease-out-expo) calc(var(--start) + var(--delay, 0ms)),
      filter 0.7s var(--ease-out-expo) calc(var(--start) + var(--delay, 0ms));
  }
  .built .build {
    opacity: 1;
    transform: none;
    filter: none;
  }

  /* The drawing zone alone carries the drafting grid. */
  .sheet :global(.drawing-zone) {
    background:
      linear-gradient(rgb(255 255 255 / 0.08) 1px, transparent 1px) 0 0 / 120px 120px,
      linear-gradient(90deg, rgb(255 255 255 / 0.08) 1px, transparent 1px) 0 0 / 120px 120px,
      linear-gradient(rgb(255 255 255 / 0.035) 1px, transparent 1px) 0 0 / 24px 24px,
      linear-gradient(90deg, rgb(255 255 255 / 0.035) 1px, transparent 1px) 0 0 / 24px 24px;
  }
  .sheet :global(.zone-pad) {
    padding: 2rem;
  }
  @media (min-width: 64rem) {
    .sheet :global(.zone-pad) {
      padding: 2.5rem 3rem 3rem;
    }
  }
  .title-block {
    display: flex;
    flex-wrap: wrap;
    gap: 1.25rem 3rem;
    padding: 1.5rem 2rem;
  }
  .strip {
    padding: 1.25rem 1.5rem 0;
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem 2.5rem;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgb(143 200 255 / 0.6);
  }

  .sheet img {
    display: block;
    max-width: none;
    filter: drop-shadow(0 0 10px rgb(108 196 255 / 0.25));
  }
  figcaption {
    margin-top: 0.9rem;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgb(143 200 255 / 0.6);
  }
  figcaption b {
    margin-right: 0.6rem;
    font-weight: 600;
    letter-spacing: 0;
    color: var(--blueprint-ink);
  }
  /* Captions clear a dimension line below the view. */
  figure:has(.dimension.horizontal) figcaption {
    margin-top: 2.25rem;
  }

  .section-number {
    font-family: var(--font-mono);
    font-size: 2.25rem;
    font-weight: 400;
    line-height: 1;
    color: var(--blueprint-ink);
    opacity: 0.55;
  }
  .section-title {
    font-family: var(--font-mono);
    font-size: 1rem;
    font-weight: 600;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: white;
  }
  .section-note {
    margin-top: 0.4rem;
    font-size: 0.85rem;
    color: rgb(143 200 255 / 0.55);
  }
  .title-label {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgb(143 200 255 / 0.5);
  }

  /* A dimension line: end ticks, the figure in a gap in the middle. */
  .dimension {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgb(143 200 255 / 0.85);
    font-family: var(--font-mono);
    font-size: 0.72rem;
  }
  .dimension::before {
    content: '';
    position: absolute;
    border: 0 solid rgb(108 196 255 / 0.5);
  }
  .dimension span {
    position: relative;
    padding: 0 0.5rem;
    /* The paper, so the figure cuts the line. */
    background: var(--blueprint-paper);
  }
  .dimension.horizontal {
    left: 0;
    right: 0;
    top: calc(100% + 0.25rem);
    height: 1rem;
  }
  .dimension.horizontal::before {
    left: 0;
    right: 0;
    top: 50%;
    height: 0.5rem;
    transform: translateY(-50%);
    border-width: 1px 1px 0;
  }
  .dimension.vertical {
    top: 0;
    bottom: 0;
    left: calc(100% + 0.75rem);
    width: 1rem;
  }
  .dimension.vertical::before {
    top: 0;
    bottom: 0;
    left: 50%;
    border-left-width: 1px;
  }
  .dimension.vertical span {
    writing-mode: vertical-rl;
    transform: rotate(180deg);
    padding: 0.5rem 0;
  }

  /* 05: the faction's seal: its emblem inked into a drafting circle. */
  .seal {
    position: relative;
    flex-shrink: 0;
    width: 8.5rem;
    aspect-ratio: 1;
    border: 1px solid rgb(108 196 255 / 0.45);
    border-radius: 50%;
  }
  .emblem {
    position: absolute;
    /* The logo images carry their own margin. */
    inset: 2%;
    z-index: 1;
    background: var(--blueprint-ink);
    mask: var(--emblem) center / contain no-repeat;
    -webkit-mask: var(--emblem) center / contain no-repeat;
    filter: drop-shadow(0 0 10px rgb(108 196 255 / 0.35));
  }
  .faction-name {
    font-family: var(--font-mono);
    font-size: 0.85rem;
    font-weight: 600;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: white;
  }
  .tagline {
    margin-top: 0.5rem;
    font-size: 0.85rem;
    font-style: italic;
    line-height: 1.5;
    color: rgb(143 200 255 / 0.85);
  }
  .story {
    margin-top: 0.75rem;
    font-size: 0.78rem;
    line-height: 1.65;
    color: rgb(143 200 255 / 0.6);
  }

  .drafting {
    animation: drafting 1.4s ease-in-out infinite;
  }
  @keyframes drafting {
    50% {
      opacity: 0.4;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .build {
      transition: none;
    }
  }
</style>

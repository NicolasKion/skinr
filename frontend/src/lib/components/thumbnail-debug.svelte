<script lang="ts">
  // A debug tool (DEBUG_TOOLS=1): the thumbnails of the livery on screen
  // (the ship's own, an in-game skin or a SKINR design, src/thumbnails.rs) in
  // both themes, a button that has the render worker draw them now, and the
  // page's Open Graph card, which shows a skin's or design's thumbnail until
  // its own render is there. The API's /api/debug answers only with the same
  // switch on.
  import { Bug, ExternalLink, LoaderCircle, X } from '@lucide/svelte';
  import type { Livery } from '#lib/components/livery-thumbnail.svelte';

  let {
    livery,
    card,
  }: {
    livery: Livery;
    /** The page's Open Graph card (`/og/…`). */
    card: string;
  } = $props();

  /** What /api/debug/thumbnails answers (src/server/debug.rs). */
  interface ThumbnailInfo {
    dna: string;
    drawn_from: string | null;
    current: boolean;
    dark: string | null;
    light: string | null;
    can_draw: boolean;
  }

  let open = $state(false);
  let info = $state<ThumbnailInfo | null>(null);
  let drawing = $state(false);
  let failure = $state<string | null>(null);
  /** How long the last drawing took, in seconds. */
  let tookSeconds = $state<number | null>(null);

  const path = $derived(
    `/api/debug/thumbnails/${livery.ship}/` +
      (livery.kind === 'skin'
        ? livery.skin
        : livery.kind === 'design'
          ? `skinr/${livery.design}`
          : 'default'),
  );
  /**
   * Moves the panel to the end of the page: the page's sections animate
   * with transforms, which would pin a fixed panel to them, and paint later
   * content over it.
   */
  function toBody(node: HTMLElement) {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  /** Bumped after a drawing, so the card is fetched again rather than from cache. */
  let cardVersion = $state(0);
  const cardUrl = $derived(cardVersion ? `${card}?v=${cardVersion}` : card);

  async function request(method: 'GET' | 'POST') {
    const response = await fetch(path, { method });
    if (!response.ok) {
      const body = await response.json().catch(() => null);
      throw new Error(body?.message ?? `${response.status} ${response.statusText}`);
    }
    return (await response.json()) as ThumbnailInfo;
  }

  // Read again whenever the panel opens or the livery changes under it.
  $effect(() => {
    if (!open) return;
    const wanted = path;
    info = null;
    failure = null;
    tookSeconds = null;
    request('GET')
      .then((answer) => {
        if (wanted === path) info = answer;
      })
      .catch((error: Error) => {
        if (wanted === path) failure = error.message;
      });
  });

  async function draw() {
    const wanted = path;
    drawing = true;
    failure = null;
    const started = performance.now();
    try {
      const answer = await request('POST');
      if (wanted !== path) return;
      info = answer;
      tookSeconds = (performance.now() - started) / 1000;
      cardVersion = Date.now();
    } catch (error) {
      if (wanted === path) failure = (error as Error).message;
    } finally {
      drawing = false;
    }
  }

  const status = $derived.by(() => {
    if (!info) return null;
    if (info.current) return { label: 'Current', tone: 'text-emerald-500' };
    if (info.drawn_from) return { label: 'Out of date', tone: 'text-amber-500' };
    return { label: 'Not drawn yet', tone: 'text-muted-foreground' };
  });
</script>

<!-- A DNA, or a design's look (hull DNA|layout JSON) with the layout folded away. -->
{#snippet key(text: string, tone: string, prefix = '')}
  {@const [dna, layout] = text.split(/\|(.*)/s)}
  <div class="font-mono text-[0.7rem] break-all {tone}">
    <p>{prefix}{dna}</p>
    {#if layout}
      <details>
        <summary class="cursor-pointer">layout, {layout.length} characters</summary>
        <p class="mt-1 max-h-32 overflow-y-auto">{layout}</p>
      </details>
    {/if}
  </div>
{/snippet}

<div class="relative">
  <button
    type="button"
    class="flex size-7 items-center justify-center rounded-full text-muted-foreground transition-colors hover:bg-foreground/10 hover:text-foreground"
    class:bg-foreground={open}
    class:text-background={open}
    title="Thumbnails (debug)"
    aria-label="Thumbnails (debug)"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <Bug class="size-4" />
  </button>

  {#if open}
    <div
      use:toBody
      class="panel fixed right-4 bottom-4 z-50 max-h-[calc(100dvh-2rem)] w-[min(22rem,calc(100vw-2rem))] overflow-y-auto rounded-3xl p-4 text-left text-sm"
      role="dialog"
      aria-label="Thumbnails"
    >
      <div class="mb-3 flex items-center justify-between gap-2">
        <p class="font-medium">
          Thumbnails <span class="text-xs font-normal text-muted-foreground">debug</span>
        </p>
        <button
          type="button"
          class="rounded-full p-1 text-muted-foreground hover:bg-foreground/10 hover:text-foreground"
          aria-label="Close"
          onclick={() => (open = false)}
        >
          <X class="size-4" />
        </button>
      </div>

      <div class="grid grid-cols-2 gap-2">
        {#each [['dark', info?.dark], ['light', info?.light]] as [theme, url] (theme)}
          <figure
            class="{theme === 'dark'
              ? 'theme-dark'
              : 'theme-light'} relative aspect-square overflow-clip rounded-2xl bg-background"
          >
            {#if url}
              <img src={url} alt="{theme} thumbnail" class="size-full object-contain" />
            {:else}
              <span
                class="absolute inset-0 flex items-center justify-center text-xs text-muted-foreground"
              >
                {info ? 'none' : '…'}
              </span>
            {/if}
            <figcaption class="absolute bottom-1.5 left-2.5 text-[0.65rem] text-muted-foreground">
              {theme}
            </figcaption>
          </figure>
        {/each}
      </div>

      <div class="mt-3 space-y-1.5">
        {#if status}
          <p class={status.tone}>
            {status.label}
            {#if tookSeconds !== null}
              <span class="text-muted-foreground">· drawn in {tookSeconds.toFixed(1)} s</span>
            {/if}
          </p>
        {/if}
        {#if info}
          {@render key(info.dna, 'text-muted-foreground')}
          {#if info.drawn_from && info.drawn_from !== info.dna}
            {@render key(info.drawn_from, 'text-muted-foreground/60', 'was ')}
          {/if}
        {/if}
        {#if failure}
          <p class="text-red-500">{failure}</p>
        {/if}
      </div>

      <button
        type="button"
        class="mt-3 flex w-full items-center justify-center gap-2 rounded-full bg-foreground px-4 py-2 font-medium text-background transition-opacity disabled:opacity-50"
        disabled={!info?.can_draw || drawing}
        onclick={draw}
      >
        {#if drawing}
          <LoaderCircle class="size-4 animate-spin" /> Drawing…
        {:else if info && !info.can_draw}
          No render worker
        {:else}
          {info?.dark ? 'Draw again' : 'Draw'}
        {/if}
      </button>
      {#if drawing}
        <p class="mt-2 text-center text-xs text-muted-foreground">
          The worker draws on the CPU; a cold ship takes a few seconds.
        </p>
      {/if}

      <div class="mt-4">
        <p class="mb-1.5 flex items-center justify-between text-xs text-muted-foreground">
          Share card
          <a
            href={cardUrl}
            target="_blank"
            rel="noopener"
            class="flex items-center gap-1 hover:text-foreground"
          >
            Open <ExternalLink class="size-3" />
          </a>
        </p>
        <img
          src={cardUrl}
          alt="Open Graph card"
          class="aspect-[1200/630] w-full rounded-2xl bg-foreground/5 object-cover"
        />
      </div>
    </div>
  {/if}
</div>

<style>
  /* Solid, unlike the site's glass: it sits over busy pages to be read. */
  .panel {
    background: color-mix(in oklab, var(--background) 94%, transparent);
    backdrop-filter: blur(24px);
    box-shadow: 0 24px 64px var(--shadow);
    border: 1px solid color-mix(in oklab, var(--foreground) 10%, transparent);
  }
</style>

<script lang="ts">
  // The explorer's filters: a segmented builder control (races, then the
  // factions that build their own ships) with a pill sliding under the
  // active one, and a search field. There are too many builders to name
  // them all in a row, so each tab is its emblem, and a caption under the
  // row names the tab hovered or focused (floating, so the row never
  // shifts). The page shows the full size
  // under its headline; the site bar shows the compact one once that row has
  // scrolled up to it. Both bind the same race and query.
  import { Search, X } from '@lucide/svelte';

  let {
    tabs,
    builder = $bindable(),
    query = $bindable(),
    compact = false,
    onfocuschange,
  }: {
    /** The tabs: a builder's key (null for all), its name and its emblem. */
    tabs: { id: string | null; name: string; logo?: string | null }[];
    builder: string | null;
    query: string;
    /** The site bar's smaller version. */
    compact?: boolean;
    /** Called when the search field gains or loses focus. */
    onfocuschange?: (focused: boolean) => void;
  } = $props();

  let input = $state<HTMLInputElement>();
  let row = $state<HTMLDivElement>();
  /** Room kept beside the picked tab when scrolling it into view, in px. */
  const SCROLL_MARGIN = 12;
  let buttons = $state<Record<string, HTMLButtonElement>>({});
  /** Bumped as tabs change size (fonts loading), so the pill keeps up. */
  let resized = $state(0);
  /** Each instance's own filter id: the page and the bar both draw one. */
  const uid = $props.id();
  const ink = `emblem-ink-${uid}`;
  /** The tab under the pointer or keyboard focus, if any. */
  let pointed = $state<string | null | undefined>(undefined);
  /** How far the row is scrolled, so the caption follows its tab. */
  let scrolled = $state(0);
  /** The tab the caption names: the one pointed at, if any. */
  const captioned = $derived(
    pointed === undefined
      ? null
      : (tabs.find((tab) => tab.id !== null && tab.id === pointed) ?? null),
  );
  const caption = $derived.by(() => {
    void resized;
    const button = captioned ? buttons[String(captioned.id)] : null;
    if (!row || !button) return null;
    return { left: row.offsetLeft + button.offsetLeft + button.offsetWidth / 2 - scrolled };
  });
  $effect(() => {
    const observer = new ResizeObserver(() => resized++);
    for (const button of Object.values(buttons)) if (button) observer.observe(button);
    return () => observer.disconnect();
  });
  const pill = $derived.by(() => {
    void resized;
    const button = buttons[String(builder)];
    return button ? { left: button.offsetLeft, width: button.offsetWidth } : null;
  });

  // Keep the picked tab in view.
  $effect(() => {
    void resized;
    const button = buttons[String(builder)];
    if (!row || !button) return;
    const left = button.offsetLeft - SCROLL_MARGIN;
    const right = button.offsetLeft + button.offsetWidth + SCROLL_MARGIN - row.clientWidth;
    if (row.scrollLeft > left) row.scrollTo({ left, behavior: 'smooth' });
    else if (row.scrollLeft < right) row.scrollTo({ left: right, behavior: 'smooth' });
  });

  /** A builder without an emblem shows its initials. */
  const initials = (name: string) =>
    name
      .split(/\s+/)
      .filter((word) => /^[A-Z]/.test(word))
      .slice(0, 2)
      .map((word) => word[0])
      .join('');

  /** Puts the cursor in the search field. */
  export function focusSearch() {
    input?.focus();
  }
</script>

<!-- Fills an emblem's shape solid in the text color: the client's logos are
     partly see-through to different degrees, so masks alone look uneven. -->
<svg class="absolute size-0" aria-hidden="true">
  <filter id={ink} color-interpolation-filters="sRGB">
    <feComponentTransfer in="SourceAlpha" result="shape">
      <feFuncA type="linear" slope="4" />
    </feComponentTransfer>
    <feFlood class="ink" />
    <feComposite in2="shape" operator="in" />
  </filter>
</svg>

<div class="filters relative flex min-w-0 items-center gap-3" class:compact>
  <div
    bind:this={row}
    class="races relative flex min-w-0 overflow-x-auto rounded-full no-scrollbar"
    onscroll={() => (scrolled = row?.scrollLeft ?? 0)}
  >
    {#if pill}
      <span class="pill" style:left="{pill.left}px" style:width="{pill.width}px"></span>
    {/if}
    {#each tabs as tab (tab.id ?? 'all')}
      <button
        type="button"
        bind:this={buttons[String(tab.id)]}
        class="race relative flex items-center rounded-full whitespace-nowrap"
        class:active={builder === tab.id}
        class:named={tab.id === null}
        aria-pressed={builder === tab.id}
        aria-label={tab.name}
        onclick={() => (builder = tab.id)}
        onpointerenter={() => (pointed = tab.id)}
        onpointerleave={() => (pointed = undefined)}
        onfocus={() => (pointed = tab.id)}
        onblur={() => (pointed = undefined)}
      >
        {#if tab.id === null}
          {tab.name}
        {:else if tab.logo}
          <img class="emblem" src={tab.logo} alt="" style:filter="url(#{ink})" />
        {:else}
          <span class="monogram">{initials(tab.name)}</span>
        {/if}
      </button>
    {/each}
  </div>

  {#if caption && captioned}
    <span class="caption" style:left="{caption.left}px" aria-hidden="true">{captioned.name}</span>
  {/if}

  <label class="search relative flex shrink-0 items-center rounded-full">
    <Search class="search-icon shrink-0 text-muted-foreground" />
    <input
      bind:this={input}
      bind:value={query}
      placeholder={compact ? 'Search ships' : 'Search ships or classes'}
      class="h-full min-w-0 flex-1 bg-transparent px-2.5 outline-none placeholder:text-muted-foreground"
      onfocus={() => onfocuschange?.(true)}
      onblur={() => onfocuschange?.(false)}
    />
    {#if query}
      <button
        type="button"
        class="mr-1.5 grid size-6 place-items-center rounded-full text-muted-foreground hover:bg-foreground/10 hover:text-foreground"
        aria-label="Clear search"
        onclick={() => (query = '')}
      >
        <X class="size-3.5" />
      </button>
    {:else}
      <kbd
        class="mr-2.5 rounded-md border border-foreground/10 px-1.5 text-[0.65rem] text-muted-foreground"
        >/</kbd
      >
    {/if}
  </label>
</div>

<style>
  /* The page's full size: glass plates of their own. */
  .races,
  .search {
    background: var(--glass);
    backdrop-filter: saturate(180%) blur(24px);
    box-shadow: inset 0 0 0 1px var(--glass-edge);
  }
  .races {
    padding: 0.25rem;
  }
  .race {
    padding: 0.375rem 0.45rem;
    font-size: 0.875rem;
    color: color-mix(in oklab, var(--foreground) 72%, transparent);
    transition: color 0.3s var(--ease-apple);
  }
  .race.named {
    padding-inline: 1rem;
  }
  /* The emblem, drawn solid in the text color (see the ink filter). */
  .emblem,
  .monogram {
    flex-shrink: 0;
    width: 1.5rem;
    height: 1.5rem;
  }
  .emblem {
    object-fit: contain;
  }
  .ink {
    flood-color: var(--foreground);
  }
  .race .emblem {
    opacity: 0.6;
    transition: opacity 0.3s var(--ease-apple);
  }
  .race:is(:hover, :focus-visible) .emblem {
    opacity: 0.85;
  }
  .race.active .emblem {
    opacity: 1;
  }
  .monogram {
    display: grid;
    place-items: center;
    font-size: 0.6rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    border-radius: 999px;
    box-shadow: inset 0 0 0 1px currentColor;
  }
  /* The name of the tab pointed at, floating under the row. */
  .caption {
    position: absolute;
    top: calc(100% + 0.5rem);
    z-index: 1;
    translate: -50% 0;
    padding: 0.2rem 0.65rem;
    border-radius: 999px;
    font-size: 0.75rem;
    white-space: nowrap;
    pointer-events: none;
    color: var(--foreground);
    background: var(--glass-strong);
    backdrop-filter: saturate(180%) blur(24px);
    box-shadow: inset 0 0 0 1px var(--glass-edge);
    transition: left 0.35s var(--ease-apple);
    animation: caption-in 0.25s var(--ease-apple);
  }
  @keyframes caption-in {
    from {
      opacity: 0;
      translate: -50% -0.25rem;
    }
  }
  .pill {
    position: absolute;
    top: 0.25rem;
    bottom: 0.25rem;
    border-radius: 999px;
    background: color-mix(in oklab, var(--foreground) 12%, transparent);
    box-shadow: inset 0 1px 0 color-mix(in oklab, var(--foreground) 8%, transparent);
    transition:
      left 0.5s var(--ease-apple),
      width 0.5s var(--ease-apple);
  }
  .search {
    height: 2.5rem;
    width: 16rem;
    font-size: 0.875rem;
  }
  .search :global(.search-icon) {
    width: 1rem;
    height: 1rem;
    margin-left: 0.875rem;
  }

  /* In the bar: no plates of their own (the bar is the glass), smaller type. */
  .compact .races,
  .compact .search {
    background: none;
    backdrop-filter: none;
    box-shadow: none;
  }
  .compact .races {
    padding: 0;
  }
  .compact .race {
    padding: 0.3rem 0.35rem;
    font-size: 0.8rem;
  }
  .compact .race.named {
    padding-inline: 0.75rem;
  }
  .compact .emblem,
  .compact .monogram {
    width: 1.125rem;
    height: 1.125rem;
  }
  .compact .pill {
    top: 0;
    bottom: 0;
    background: color-mix(in oklab, var(--foreground) 9%, transparent);
  }
  .compact .search {
    height: 2rem;
    width: 13rem;
    font-size: 0.8rem;
    background: color-mix(in oklab, var(--foreground) 6%, transparent);
  }
  .compact .search :global(.search-icon) {
    width: 0.875rem;
    height: 0.875rem;
    margin-left: 0.7rem;
  }
</style>

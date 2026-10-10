<script lang="ts">
  // The explorer's filters: a segmented race control with a pill sliding
  // under the active race, and a search field. The page shows the full size
  // under its headline; the site bar shows the compact one once that row has
  // scrolled up to it. Both bind the same race and query.
  import { Search, X } from '@lucide/svelte';

  let {
    races,
    race = $bindable(),
    query = $bindable(),
    compact = false,
    onfocuschange,
  }: {
    races: { id: number | null; name: string }[];
    race: number | null;
    query: string;
    /** The site bar's smaller version. */
    compact?: boolean;
    /** Called when the search field gains or loses focus. */
    onfocuschange?: (focused: boolean) => void;
  } = $props();

  let input = $state<HTMLInputElement>();
  let buttons = $state<Record<string, HTMLButtonElement>>({});
  const pill = $derived.by(() => {
    const button = buttons[String(race)];
    return button ? { left: button.offsetLeft, width: button.offsetWidth } : null;
  });

  /** Puts the cursor in the search field. */
  export function focusSearch() {
    input?.focus();
  }
</script>

<div class="filters flex min-w-0 items-center gap-3" class:compact>
  <div class="races relative flex min-w-0 overflow-x-auto rounded-full no-scrollbar">
    {#if pill}
      <span class="pill" style:left="{pill.left}px" style:width="{pill.width}px"></span>
    {/if}
    {#each races as tab (tab.id ?? 'all')}
      <button
        type="button"
        bind:this={buttons[String(tab.id)]}
        class="race relative rounded-full whitespace-nowrap"
        class:active={race === tab.id}
        aria-pressed={race === tab.id}
        onclick={() => (race = tab.id)}
      >
        {tab.name}
      </button>
    {/each}
  </div>

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
        class="mr-1.5 grid size-6 place-items-center rounded-full text-muted-foreground hover:bg-white/10 hover:text-foreground"
        aria-label="Clear search"
        onclick={() => (query = '')}
      >
        <X class="size-3.5" />
      </button>
    {:else}
      <kbd
        class="mr-2.5 rounded-md border border-white/10 px-1.5 text-[0.65rem] text-muted-foreground"
        >/</kbd
      >
    {/if}
  </label>
</div>

<style>
  /* The page's full size: glass plates of their own. */
  .races,
  .search {
    background: oklch(0.18 0 0 / 0.55);
    backdrop-filter: saturate(180%) blur(24px);
    box-shadow: inset 0 0 0 1px oklch(1 0 0 / 0.09);
  }
  .races {
    padding: 0.25rem;
  }
  .race {
    padding: 0.375rem 1rem;
    font-size: 0.875rem;
    color: color-mix(in oklab, var(--foreground) 55%, transparent);
    transition: color 0.3s var(--ease-apple);
  }
  .race:hover {
    color: color-mix(in oklab, var(--foreground) 85%, transparent);
  }
  .race.active {
    color: var(--foreground);
  }
  .pill {
    position: absolute;
    top: 0.25rem;
    bottom: 0.25rem;
    border-radius: 999px;
    background: rgb(255 255 255 / 0.12);
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.08);
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
    padding: 0.3rem 0.75rem;
    font-size: 0.8rem;
  }
  .compact .pill {
    top: 0;
    bottom: 0;
    background: oklch(1 0 0 / 0.09);
  }
  .compact .search {
    height: 2rem;
    width: 13rem;
    font-size: 0.8rem;
    background: oklch(1 0 0 / 0.06);
  }
  .compact .search :global(.search-icon) {
    width: 0.875rem;
    height: 0.875rem;
    margin-left: 0.7rem;
  }
</style>

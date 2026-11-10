<script lang="ts">
  // A SKINR design as a card: the ship wearing it, large (the design's
  // thumbnail, or the bare hull until it has one) on a glow in the hull's
  // race colour, its faction's emblem in the top right corner, and at the
  // bottom left its hexagon, name, hull and designer, and whatever the page
  // adds below (price, licenses, a listing's state). It links to the design
  // on its ship's page. Beside the preview panel (a wide screen) a first
  // click puts the design on the stage instead, like the explorer's cards do
  // a ship; a click on the design already there opens it, and so does any
  // tap on a phone, which has no panel.
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import { isPlainClick } from '#lib/clicks.js';
  import FactionEmblem from '#lib/components/faction-emblem.svelte';
  import LiveryThumbnail from '#lib/components/livery-thumbnail.svelte';
  import SkinIcon from '#lib/components/skin-icon.svelte';
  import { raceTint } from '#lib/eve.js';
  import { skinrIcon } from '#lib/skinr.js';
  import { stage } from '#lib/stage.svelte.js';
  import type { Catalog, SkinrComponent, SkinrDesign } from '#lib/types.js';
  import { designHref } from '#lib/urls.js';
  import { viewport } from '#lib/viewport.svelte.js';

  let {
    design,
    components,
    shipName,
    selected = false,
    onselect,
    children,
  }: {
    design: SkinrDesign;
    components: Record<string, SkinrComponent>;
    /** The hull's name, when the catalog has it. */
    shipName: string | null;
    /** Whether the design is the one on the stage. */
    selected?: boolean;
    /** Puts the design on the stage (the preview panel). */
    onselect: () => void;
    /** Extra lines under the name. */
    children?: Snippet;
  } = $props();

  /** The emblem's strength: clearly there, but quieter than the ship. */
  const EMBLEM_OPACITY = 0.85;

  const icon = $derived(skinrIcon(design, components));
  const catalog = $derived(page.data.catalog as Catalog | undefined);
  const ship = $derived(catalog?.ships.find((candidate) => candidate.id === design.ship_type_id));
  const tint = $derived(raceTint(ship?.race_id ?? null));
  const emblem = $derived(
    ship?.faction_id != null
      ? (catalog?.factions.find((faction) => faction.id === ship.faction_id)?.logo_url ?? null)
      : null,
  );
  /** The design on its ship's page; none for a hull the catalog does not have. */
  const href = $derived(ship ? designHref(ship, design) : null);

  function onclick(event: MouseEvent & { currentTarget: HTMLElement }) {
    // A mouse click lets go of focus; keyboard clicks (detail 0) keep it.
    if (event.detail > 0) event.currentTarget.blur();
    const previews = viewport.wide && !selected && isPlainClick(event);
    if (previews || !href) {
      event.preventDefault();
      onselect();
      return;
    }
    // Opening the page: nothing here morphs into its title.
    stage.morphId = null;
  }
</script>

<svelte:element
  this={href ? 'a' : 'button'}
  {href}
  type={href ? undefined : 'button'}
  role={href ? undefined : 'button'}
  class="tile group relative block aspect-[4/3] w-full min-w-0 overflow-hidden rounded-[1.75rem] text-left"
  class:selected
  style:--race={tint}
  aria-current={selected ? 'true' : undefined}
  {onclick}
>
  <div class="backdrop absolute inset-0"></div>
  <LiveryThumbnail
    livery={{ kind: 'design', ship: design.ship_type_id, design: design.id }}
    width={512}
    alt={shipName ?? ''}
    class="absolute inset-x-[6%] top-[2%] h-[78%] w-[88%] transition-transform duration-700 ease-[var(--ease-apple)] group-hover:-translate-y-[2%] group-hover:scale-[1.06]"
  />
  {#if emblem}
    <FactionEmblem
      url={emblem}
      {tint}
      opacity={EMBLEM_OPACITY}
      class="absolute top-4 right-4 size-9"
    />
  {/if}
  <div class="caption absolute inset-x-0 bottom-0 flex items-end gap-3 p-4 pt-10">
    <SkinIcon
      colors={icon.colors}
      patterns={icon.patterns}
      {selected}
      class="size-12 shrink-0 transition-transform duration-500 ease-[var(--ease-apple)] group-hover:-translate-y-0.5"
    />
    <span class="min-w-0 flex-1">
      <span class="block truncate font-medium">{design.name}</span>
      <span class="block truncate text-xs text-muted-foreground">
        {[shipName, design.creator.name].filter(Boolean).join(' · ')}
      </span>
      {#if children}
        <span class="mt-0.5 block truncate text-xs text-foreground/70">{@render children()}</span>
      {/if}
    </span>
  </div>
</svelte:element>

<style>
  .tile {
    background: color-mix(in oklab, var(--foreground) 3%, transparent);
    box-shadow:
      0 30px 60px -30px var(--shadow),
      inset 0 0 0 1px color-mix(in oklab, var(--foreground) 4%, transparent);
    transition: box-shadow 0.3s var(--ease-apple);
  }
  .tile.selected {
    box-shadow:
      0 30px 60px -30px var(--shadow),
      inset 0 0 0 1.5px color-mix(in oklab, var(--foreground) 50%, transparent);
  }
  /* The race's glow behind the ship, as on the explorer's cards. */
  .backdrop {
    background: radial-gradient(
      70% 70% at 50% 38%,
      color-mix(in oklab, var(--race) var(--race-backdrop-core), var(--background)),
      color-mix(in oklab, var(--race) var(--race-backdrop-edge), var(--background))
    );
  }
  /* Keeps the text readable where the ship runs under it. */
  .caption {
    background: linear-gradient(
      to top,
      color-mix(in oklab, var(--background) 85%, transparent),
      transparent
    );
  }
</style>

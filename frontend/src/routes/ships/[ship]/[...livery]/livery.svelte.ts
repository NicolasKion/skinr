// The livery on a ship page's ship: which one the path names (an in-game
// skin, a SKINR design, or the hull's own colors), and picking another. The
// pick lives in the path, so a link or a reload keeps it.
import { goto } from '$app/navigation';
import { page } from '$app/state';
import type { Livery } from '#lib/components/livery-thumbnail.svelte';
import { skinrLook } from '#lib/skinr.js';
import { stage } from '#lib/stage.svelte.js';
import type { Colors, IconColors, ShipDetail, ShipSkinrs, SkinrDesign } from '#lib/types.js';
import { designHref, parseLivery, shipHref, skinHref, slugify } from '#lib/urls.js';
import { summaryOf } from './related';

/** The name of the hull's own colors, where liveries are listed. */
const STANDARD_ISSUE = 'Standard issue';

/** A livery in the pickers: the hull's own (`id` null) or an in-game skin. */
export interface LiveryOption {
  id: number | null;
  name: string;
  colors: Colors;
  icon_url: string | null;
  icon_colors: IconColors | null;
}

export type LiveryState = ReturnType<typeof useLivery>;

/**
 * Reads the page's livery and keeps the stage showing the ship in it. Call
 * while a component initialises.
 */
export function useLivery(source: () => { ship: ShipDetail; skinrs: ShipSkinrs }) {
  const ship = $derived(source().ship);
  const skinrs = $derived(source().skinrs);

  /** The livery the path names: `…/skins/{name}`, `…/skinr/{name}-{id}` or none. */
  const named = $derived(parseLivery(page.params.livery ?? '') ?? { kind: 'none' as const });
  /** The selected in-game skin, or null for the ship's own livery. */
  const skin = $derived(
    named.kind === 'skin'
      ? (ship.skins.find((candidate) => slugify(candidate.name) === named.slug) ?? null)
      : null,
  );
  /** The chosen player design, which replaces the livery. */
  const design = $derived(
    named.kind === 'skinr'
      ? (skinrs.designs.find((candidate) => candidate.id.startsWith(named.prefix)) ?? null)
      : null,
  );
  const look = $derived(
    design ? skinrLook(ship.dna, design, skinrs.components, skinrs.slot_materials) : null,
  );
  const dna = $derived(look?.dna ?? skin?.dna ?? ship.dna);
  const options = $derived<LiveryOption[]>([
    { id: null, name: STANDARD_ISSUE, colors: ship.colors, icon_url: null, icon_colors: null },
    ...ship.skins,
  ]);

  // The stage shows this ship in the chosen livery; where it sits is up to
  // the page's chapters.
  $effect(() => {
    const summary = summaryOf(ship);
    stage.morphId = summary.id;
    stage.show(summary, dna, stage.direction, look?.pattern ?? null);
  });

  /**
   * Shows an in-game skin, or the hull's own colors with null. The carousel
   * slides it in from the side it sits on in the list, or from the side an
   * arrow key points (stepping wraps around).
   */
  function pickSkin(id: number | null, direction?: 1 | -1) {
    const current = skin?.id ?? null;
    if (id === current) return;
    const order = options.map((option) => option.id);
    stage.direction = direction ?? (order.indexOf(id) >= order.indexOf(current) ? 1 : -1);
    const picked = id === null ? null : ship.skins.find((candidate) => candidate.id === id);
    goto(picked ? skinHref(ship, picked) : shipHref(ship), { replace: true, reset: false });
  }

  /** Shows a player design; it slides in like a skin does. */
  function pickDesign(picked: SkinrDesign, direction?: 1 | -1) {
    if (picked.id === design?.id) return;
    const order = skinrs.designs.map((candidate) => candidate.id);
    const current = design ? order.indexOf(design.id) : -1;
    stage.direction = direction ?? (order.indexOf(picked.id) >= current ? 1 : -1);
    goto(designHref(ship, picked), { replace: true, reset: false });
  }

  return {
    /** The in-game skin on the ship, if any. */
    get skin() {
      return skin;
    },
    /** The SKINR design on the ship, if any. */
    get design() {
      return design;
    },
    /** The DNA the ship is built from in this livery. */
    get dna() {
      return dna;
    },
    /** The hull's own colors and every in-game skin, in list order. */
    get options() {
      return options;
    },
    /** The in-game livery's name (the hull's own when a design is worn). */
    get name() {
      return skin?.name ?? STANDARD_ISSUE;
    },
    /** The livery on the ship, as the thumbnails name it (the debug tools). */
    get onShip(): Livery {
      if (design) return { kind: 'design', ship: ship.id, design: design.id };
      return skin
        ? { kind: 'skin', ship: ship.id, skin: skin.id }
        : { kind: 'ship', ship: ship.id };
    },
    pickSkin,
    pickDesign,
    /**
     * Steps to the next (1) or previous (-1) look, wrapping around: through
     * the designs with `designs` while one is worn, else the in-game liveries.
     */
    step(direction: 1 | -1, designs: boolean) {
      const wrap = (index: number, length: number) => (index + direction + length) % length;
      if (designs && design) {
        const all = skinrs.designs;
        pickDesign(all[wrap(all.indexOf(design), all.length)], direction);
        return;
      }
      const order = options.map((option) => option.id);
      pickSkin(order[wrap(order.indexOf(skin?.id ?? null), order.length)] ?? null, direction);
    },
  };
}

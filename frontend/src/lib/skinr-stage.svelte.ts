// The SKINR pages (the Paragon Hub, a character's own) show the design picked
// in a list on the shared stage, docked in `skinr-dock.svelte` at the right.
// The pick lives in the URL (`?skinr=`), so a link or a reload keeps it.
import { goto } from '$app/navigation';
import { page } from '$app/state';
import { skinrLookOn } from './skinr';
import { stage, useScene } from './stage.svelte';
import type { FactionSlotMaterials, ShipSummary, SkinrComponent, SkinrDesign } from './types';

/** The query parameter holding the picked design. */
const PICK_PARAM = 'skinr';

export interface SkinrStageSource {
  /** Designs in the order the page lists them; the first is shown by default. */
  designs: SkinrDesign[];
  components: Record<string, SkinrComponent>;
  slotMaterials: FactionSlotMaterials;
  ships: Map<number, ShipSummary>;
}

/**
 * Docks the stage for a SKINR page and keeps the picked design on it. Call
 * while a component initialises; returns the pick and a way to change it.
 */
export function skinrStage(source: () => SkinrStageSource) {
  const pickedId = $derived(page.url.searchParams.get(PICK_PARAM));
  const picked = $derived.by(() => {
    const { designs, ships } = source();
    const onAHull = designs.filter((design) => ships.has(design.ship_type_id));
    return onAHull.find((design) => design.id === pickedId) ?? onAHull[0] ?? null;
  });
  const ship = $derived(picked ? (source().ships.get(picked.ship_type_id) ?? null) : null);

  // The stage docks in the preview panel; a design for another hull slides
  // in like a livery. With nothing to show (My skins logged out) the page
  // has no panel and the stage goes with the last page.
  useScene(() => (picked ? { place: 'peek', slideShips: true } : null));
  const look = $derived.by(() => {
    if (!picked || !ship) return null;
    const { components, slotMaterials } = source();
    return skinrLookOn(ship, picked, components, slotMaterials);
  });
  $effect(() => {
    if (!look || !ship) return;
    stage.morphId = null;
    stage.show(ship, look.dna, stage.direction, look.pattern);
  });
  /**
   * What the panel shows: the hull on screen, and the picked design once its
   * look is on it. Arriving from a ship's page, that ship glides into the
   * panel first and the pick follows, so the labels wait for it.
   */
  const docked = $derived({
    ship: stage.onScreen,
    design: look && stage.onScreenDna === look.dna ? picked : null,
  });

  return {
    get design() {
      return picked;
    },
    get ship() {
      return ship;
    },
    get docked() {
      return docked;
    },
    /** Shows a design; it slides in from the side it sits on in `order`. */
    pick(design: SkinrDesign, order: SkinrDesign[]) {
      if (design.id === picked?.id) return;
      const ids = order.map((candidate) => candidate.id);
      const current = picked ? ids.indexOf(picked.id) : -1;
      stage.direction = ids.indexOf(design.id) >= current ? 1 : -1;
      const url = new URL(page.url.href);
      url.searchParams.set(PICK_PARAM, design.id);
      goto(url, { replace: true, reset: false });
    },
  };
}

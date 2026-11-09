// The SKINR pages (the Paragon Hub, a character's own) show the design picked
// in a list on the shared stage, docked in `skinr-dock.svelte` at the right.
// The pick lives in the URL (`?skinr=`), so a link or a reload keeps it.
import { goto } from '$app/navigation';
import { page } from '$app/state';
import { skinrLookOn } from './skinr';
import { CENTER, stage } from './stage.svelte';
import type { FactionSlotMaterials, ShipSummary, SkinrComponent, SkinrDesign } from './types';
import { HERO_POSE } from './viewer/poses';

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

  $effect(() => {
    stage.mode = 'dock';
    stage.placement = CENTER;
    stage.pose = HERO_POSE;
    stage.dim = 0;
    stage.hologram = 0;
    stage.dockName = 'dock';
    stage.dockRadius = 32;
  });
  // A design for another hull slides in like a livery.
  $effect(() => {
    stage.slideShips = true;
    return () => {
      stage.slideShips = false;
    };
  });
  $effect(() => {
    if (!picked || !ship) return;
    const { components, slotMaterials } = source();
    const look = skinrLookOn(ship, picked, components, slotMaterials);
    stage.morphId = null;
    stage.show(ship, look.dna, stage.direction, look.pattern);
  });

  return {
    get design() {
      return picked;
    },
    get ship() {
      return ship;
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

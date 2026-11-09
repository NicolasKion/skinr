// A SKINR design as the SOF builds it, from the API: the same skinr.ts the
// site uses, so a rendered card shows what the page shows.

import { skinrLook, type SkinrLook } from '../../src/lib/skinr.ts';

/** A design id: 64 hex digits. */
export const DESIGN_ID = /^[0-9a-f]{64}$/;

/**
 * The look of design `designId` on ship `shipId`, or null when the API does
 * not know the ship or the design is not one of its designs.
 */
export async function designLook(
  api: string,
  designId: string,
  shipId: number,
): Promise<SkinrLook | null> {
  const [ship, skinrs] = await Promise.all([
    fetch(`${api}/api/ships/${shipId}`).then((response) => (response.ok ? response.json() : null)),
    fetch(`${api}/api/ships/${shipId}/skinrs?include=${designId}`).then((response) =>
      response.ok ? response.json() : null,
    ),
  ]);
  if (!ship || !skinrs) return null;
  const design = skinrs.designs.find((candidate: { id: string }) => candidate.id === designId);
  if (!design) return null;
  return skinrLook(ship.dna, design, skinrs.components, skinrs.slot_materials);
}

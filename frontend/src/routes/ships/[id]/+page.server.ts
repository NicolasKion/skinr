import { error } from '@sveltejs/kit';
import { apiGet } from '$lib/server/api';
import type { ShipDetail, ShipSkinrs } from '$lib/types';
import type { PageServerLoad } from './$types';

/** What the page shows when the SKINR designs cannot be loaded. */
const NO_SKINRS: ShipSkinrs = { slot_materials: {}, designs: [], components: {} };

export const load: PageServerLoad = async ({ fetch, params }) => {
  if (!/^\d+$/.test(params.id)) error(404, 'Ship not found.');
  // The player designs are extra: the page loads without them.
  const skinrs = fetch(`/api/ships/${params.id}/skinrs`)
    .then((response) => (response.ok ? (response.json() as Promise<ShipSkinrs>) : NO_SKINRS))
    .catch(() => NO_SKINRS);
  const [ship, designs] = await Promise.all([
    apiGet<ShipDetail>(fetch, `/api/ships/${params.id}`),
    skinrs,
  ]);
  return { ship, skinrs: designs };
};

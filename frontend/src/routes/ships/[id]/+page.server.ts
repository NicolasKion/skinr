import { error } from '@sveltejs/kit';
import { apiGet } from '$lib/server/api';
import type { ShipDetail } from '$lib/types';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, params }) => {
  if (!/^\d+$/.test(params.id)) error(404, 'Ship not found.');
  return { ship: await apiGet<ShipDetail>(fetch, `/api/ships/${params.id}`) };
};

import { error } from '@sveltejs/kit';
import { dev } from '$app/env';
import { apiGet } from '#lib/server/api.js';
import type { ShipDetail } from '#lib/types.js';
import type { PageServerLoad } from './$types';

/** Ships with many skins across the races (Gila, Hurricane, Caracal, Coercer). */
const DEFAULT_SHIPS = '17715,24702,621,16236';

// A dev-only page that puts each in-game SKIN icon next to our vector redraw.
export const load: PageServerLoad = async ({ fetch, url }) => {
  if (!dev) error(404, 'Not found.');

  const ids = (url.searchParams.get('ships') ?? DEFAULT_SHIPS)
    .split(',')
    .filter((id) => /^\d+$/.test(id));
  const ships = await Promise.all(ids.map((id) => apiGet<ShipDetail>(fetch, `/api/ships/${id}`)));
  return { ships };
};

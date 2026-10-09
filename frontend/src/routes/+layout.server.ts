import { apiGet } from '$lib/server/api';
import type { Catalog } from '$lib/types';
import type { LayoutServerLoad } from './$types';

// The whole catalog is a few hundred rows; every page uses it (the explorer,
// a ship's related ships), so it loads once with the layout.
export const load: LayoutServerLoad = async ({ fetch }) => ({
  catalog: await apiGet<Catalog>(fetch, '/api/ships'),
});

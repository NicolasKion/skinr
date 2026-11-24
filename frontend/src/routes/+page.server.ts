import { filesAhead } from '#lib/server/files-ahead.js';
import { pickFeatured } from './featured';
import type { PageServerLoad } from './$types';

// The explorer opens on a featured ship. The server picks it, so the page it
// sends can already name that ship's game files.
export const load: PageServerLoad = async ({ fetch, parent, isDataRequest }) => {
  const { catalog } = await parent();
  const featured = pickFeatured(catalog.ships, Math.random());
  return {
    featured: featured?.id ?? null,
    // Only in the HTML the server sends (see the ship page's load).
    filesAhead:
      featured && !isDataRequest ? await filesAhead(fetch, featured.id, featured.dna) : [],
  };
};

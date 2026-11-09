import { error, redirect } from '@sveltejs/kit';
import { apiGet } from '#lib/server/api.js';
import { hubQuery } from '#lib/server/hub.js';
import type { HubPage } from '#lib/types.js';
import { designerHref, designerId } from '#lib/urls.js';
import type { PageServerLoad } from './$types';

/**
 * One designer's line-up, at `/skinr/designer/{name}-{id}`. The id decides;
 * a name that is out of date (or missing) leads to the current one.
 */
export const load: PageServerLoad = async ({ fetch, params, url }) => {
  const id = designerId(params.designer);
  if (id === null) error(404, 'Designer not found.');
  const hub = await apiGet<HubPage>(fetch, `/api/skinr?${hubQuery(url, id)}`);
  // The API answers any id; one with no name and nothing on sale is nobody we know.
  if (!hub.designer || (!hub.designer.name && hub.designer.designs === 0)) {
    error(404, 'Designer not found.');
  }
  const canonical = designerHref(hub.designer);
  if (url.pathname !== canonical) redirect(308, `${canonical}${url.search}`);
  return { hub };
};

import { redirect } from '@sveltejs/kit';
import { apiGet } from '#lib/server/api.js';
import type { HubPage } from '#lib/types.js';
import { hubQuery } from '#lib/server/hub.js';
import { designerHref } from '#lib/urls.js';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ fetch, url }) => {
  // A designer's line-up has its own page now; old links lead there.
  const designer = url.searchParams.get('designer');
  if (designer && /^\d+$/.test(designer)) {
    const rest = new URLSearchParams(url.searchParams);
    rest.delete('designer');
    const search = rest.size ? `?${rest}` : '';
    redirect(308, `${designerHref({ id: Number(designer), name: null })}${search}`);
  }
  return { hub: await apiGet<HubPage>(fetch, `/api/skinr?${hubQuery(url)}`) };
};

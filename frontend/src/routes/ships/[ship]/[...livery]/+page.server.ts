import { error, redirect } from '@sveltejs/kit';
import { apiGet } from '#lib/server/api.js';
import type { ShipDetail, ShipSkinrs } from '#lib/types.js';
import { designHref, parseLivery, shipHref, skinHref, slugify } from '#lib/urls.js';
import type { PageServerLoad } from './$types';

/** What the page shows when the SKINR designs cannot be loaded. */
const NO_SKINRS: ShipSkinrs = { slot_materials: {}, designs: [], components: {} };

/**
 * A ship's page, at its name (`/ships/kronos`), optionally wearing one of its
 * in-game skins (`…/skins/headhunter`) or a SKINR design (`…/skinr/{name}-{id}`).
 * A type id, or the old `?skin=`/`?skinr=` query, leads to the same URL.
 * Picking another livery on the page only changes the path, which must not
 * load the page again, so the path is read untracked.
 */
export const load: PageServerLoad = async ({ fetch, params, parent, url, untrack }) => {
  const { catalog } = await parent();
  const [rest, search] = untrack(() => [params.livery, url.searchParams] as const);
  const byId = /^\d+$/.test(params.ship)
    ? catalog.ships.find((ship) => ship.id === Number(params.ship))
    : undefined;
  if (byId) redirect(308, `${shipHref(byId)}${rest ? `/${rest}` : ''}${untrack(() => url.search)}`);
  const found = catalog.ships.find((ship) => slugify(ship.name) === params.ship);
  if (!found) error(404, 'Ship not found.');
  const livery = parseLivery(rest);
  if (!livery) error(404, 'Livery not found.');

  // A design named by the path (one the visitor owns, say) comes along even
  // when it is not on sale; so does one an old `?skinr=` link names.
  const wanted = livery.kind === 'skinr' ? livery.prefix : search.get('skinr');
  const include = wanted ? `?include=${encodeURIComponent(wanted)}` : '';
  // The player designs are extra: the page loads without them.
  const skinrs = fetch(`/api/ships/${found.id}/skinrs${include}`)
    .then((response) => (response.ok ? (response.json() as Promise<ShipSkinrs>) : NO_SKINRS))
    .catch(() => NO_SKINRS);
  const [ship, designs] = await Promise.all([
    apiGet<ShipDetail>(fetch, `/api/ships/${found.id}`),
    skinrs,
  ]);

  // Links from before liveries had their own URLs.
  const oldSkin = Number(search.get('skin'));
  const skinById = oldSkin ? ship.skins.find((skin) => skin.id === oldSkin) : undefined;
  if (skinById) redirect(308, skinHref(ship, skinById));
  const oldDesign = search.get('skinr');
  const designById = oldDesign && designs.designs.find((design) => design.id === oldDesign);
  if (designById) redirect(308, designHref(ship, designById));

  let canonical = shipHref(ship);
  if (livery.kind === 'skin') {
    const skin = ship.skins.find((candidate) => slugify(candidate.name) === livery.slug);
    if (!skin) error(404, 'Skin not found.');
    canonical = skinHref(ship, skin);
  } else if (livery.kind === 'skinr') {
    const design = designs.designs.find((candidate) => candidate.id.startsWith(livery.prefix));
    if (!design) error(404, 'Design not found.');
    canonical = designHref(ship, design);
  }
  // A design renamed since its link was shared still lands, under its name now.
  if (untrack(() => url.pathname) !== canonical) redirect(308, canonical);
  return { ship, skinrs: designs };
};

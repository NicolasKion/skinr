// What search engines and link previews say about a ship page.
import { firstSentence } from '#lib/eve.js';
import type { ShipDetail, Skin, SkinrDesign } from '#lib/types.js';
import { designHref, shipHref, skinHref } from '#lib/urls.js';

export interface ShipSeo {
  title: string;
  description: string;
  /** The share card, under `/og/`. */
  image: string;
  /** The breadcrumb trail: name and path. */
  crumbs: [string, string][];
}

const plex = new Intl.NumberFormat('en-US');

/** The page's head for a ship, or for the livery on it. */
export function shipSeo(ship: ShipDetail, skin: Skin | null, design: SkinrDesign | null): ShipSeo {
  const crumbs: [string, string][] = [
    ['Ships', '/'],
    [ship.name, shipHref(ship)],
  ];
  if (design) {
    const by = design.creator.name ? ` by ${design.creator.name}` : '';
    const price =
      design.min_price_plex !== null
        ? ` From ${plex.format(design.min_price_plex)} PLEX on the Paragon Hub.`
        : '';
    return {
      title: `${design.name}, a SKINR for the ${ship.name}`,
      description: `${design.name}${by}: a SKINR design for the ${ship.name}, on the real hull in 3D.${price}`,
      image: `/og/skinr/${design.id}.png`,
      crumbs: [...crumbs, [design.name, designHref(ship, design)]],
    };
  }
  if (skin) {
    return {
      title: `${skin.name} ${ship.name} skin`,
      description: `The ${skin.name} skin on the ${ship.name}, on the real hull in 3D. Compare it with every other skin and SKINR design for the ${ship.name}.`,
      image: `/og/ships/${ship.id}/skins/${skin.id}.png`,
      crumbs: [...crumbs, [skin.name, skinHref(ship, skin)]],
    };
  }
  const about = firstSentence(ship.description);
  return {
    title: `${ship.name} skins`,
    description: `Every skin and SKINR design for the ${ship.name}, on the real hull in 3D.${about ? ` ${about}` : ''}`,
    image: `/og/ships/${ship.id}.png`,
    crumbs,
  };
}

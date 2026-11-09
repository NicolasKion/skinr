// The site's readable URLs: a ship by its name (`/ships/kronos`), one of its
// in-game skins by name (`/ships/kronos/skins/headhunter`), a SKINR design by
// name and the start of its id (`/ships/kronos/skinr/deep-void-109feb1d1a2b`),
// a designer by name and character id (`/skinr/designer/nicolas-kion-95569887`).
// Ship names are unique, and skin names unique per ship, so neither needs an
// id; design and character names are not, so their id rides along and is
// what resolves. The API's sitemap (src/urls.rs) builds the same URLs.

/** A name as a URL segment: lowercase letters and digits, dashes between words. */
export function slugify(name: string): string {
  return name
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/['’]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

/** A ship's page. */
export function shipHref(ship: { name: string }, query = ''): string {
  return `/ships/${slugify(ship.name)}${query}`;
}

/** How many characters of a design's id its URL carries (eight are already unique). */
export const DESIGN_ID_CHARS = 12;

/** One of a ship's in-game skins, on the ship's page. */
export function skinHref(ship: { name: string }, skin: { name: string }): string {
  return `${shipHref(ship)}/skins/${slugify(skin.name)}`;
}

/** A SKINR design, on its ship's page. */
export function designHref(ship: { name: string }, design: { id: string; name: string }): string {
  const name = slugify(design.name);
  const id = design.id.slice(0, DESIGN_ID_CHARS);
  return `${shipHref(ship)}/skinr/${name ? `${name}-` : ''}${id}`;
}

/** The livery a ship page's path names after the ship: none, a skin or a design. */
export type Livery =
  | { kind: 'none' }
  | { kind: 'skin'; slug: string }
  | { kind: 'skinr'; prefix: string };

/** Reads the path after `/ships/{ship}/`; null for one that names nothing. */
export function parseLivery(rest: string): Livery | null {
  if (rest === '') return { kind: 'none' };
  const [kind, segment, ...extra] = rest.split('/');
  if (!segment || extra.length) return null;
  if (kind === 'skins') return { kind: 'skin', slug: segment };
  const prefix = kind === 'skinr' ? /(?:^|-)([0-9a-f]{8,64})$/.exec(segment)?.[1] : undefined;
  return prefix ? { kind: 'skinr', prefix } : null;
}

/** A designer's page on the Paragon Hub. */
export function designerHref(designer: { id: number; name: string | null }): string {
  const name = designer.name ? slugify(designer.name) : '';
  return `/skinr/designer/${name ? `${name}-` : ''}${designer.id}`;
}

/** The character id at the end of a designer segment, or null. */
export function designerId(segment: string): number | null {
  const match = /(?:^|-)(\d+)$/.exec(segment);
  return match ? Number(match[1]) : null;
}

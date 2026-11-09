// The Paragon Hub's API query, shared by /skinr and a designer's page.

/** The filters the hub passes on to the API, as they appear in the page's URL. */
const HUB_FILTERS = ['sort', 'q', 'ship', 'page'] as const;

/** The API query for the hub's filters in `url`, plus a designer if given. */
export function hubQuery(url: URL, designer: number | null = null): URLSearchParams {
  const query = new URLSearchParams();
  for (const name of HUB_FILTERS) {
    const value = url.searchParams.get(name);
    if (value) query.set(name, value);
  }
  if (designer !== null) query.set('designer', String(designer));
  return query;
}

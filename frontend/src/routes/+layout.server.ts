import { DEBUG_TOOLS, PUBLIC_ORIGIN } from '$app/env/private';
import { apiGet } from '#lib/server/api.js';
import { PEEK_COOKIE, parsePeek } from '#lib/peek.js';
import type { Catalog, Me } from '#lib/types.js';
import type { LayoutServerLoad } from './$types';

/** Who is shown as logged in when the API cannot say. */
const NOBODY: Me = { character: null, login: null };

// The whole catalog is a few hundred rows; every page uses it (the explorer,
// a ship's related ships), so it loads once with the layout. So do the
// logged-in character, for the site bar, and the explorer's stage width, so
// the server renders it at the visitor's width.
export const load: LayoutServerLoad = async ({ fetch, cookies, url, untrack }) => {
  const me = fetch('/api/me')
    .then((response) => (response.ok ? (response.json() as Promise<Me>) : NOBODY))
    .catch(() => NOBODY);
  const [catalog, who] = await Promise.all([apiGet<Catalog>(fetch, '/api/ships'), me]);
  // Canonical links and Open Graph tags need absolute URLs (#lib/components/seo.svelte).
  const origin = PUBLIC_ORIGIN ?? untrack(() => url.origin);
  return {
    catalog,
    me: who,
    peek: parsePeek(cookies.get(PEEK_COOKIE)),
    origin,
    // The debug tools (#lib/components/thumbnail-debug.svelte), when DEBUG_TOOLS=1.
    debug: DEBUG_TOOLS,
  };
};

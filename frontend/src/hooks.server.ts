import type { Handle, HandleFetch } from '@sveltejs/kit/hooks';
import { AXUM_URL } from '$app/env/private';
import { THEME_COOKIE, parseTheme } from '#lib/theme.js';

// The page renders in the visitor's theme (a class on <html>, app.html).
export const handle: Handle = ({ event, resolve }) => {
  const theme = parseTheme(event.cookies.get(THEME_COOKIE));
  return resolve(event, {
    transformPageChunk: ({ html }) => html.replace('%theme%', theme),
  });
};

// Server-side loads reach Axum directly; the browser goes through the
// shared-origin proxy instead.
export const handleFetch: HandleFetch = async ({ event, request, fetch }) => {
  const url = new URL(request.url);
  if (url.origin === event.url.origin && url.pathname.startsWith('/api')) {
    request = new Request(new URL(url.pathname + url.search, AXUM_URL), request);
  }

  return fetch(request);
};

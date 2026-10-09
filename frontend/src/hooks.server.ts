import type { HandleFetch } from '@sveltejs/kit';
import { env } from '$env/dynamic/private';

// Server-side loads reach Axum directly; the browser goes through the
// shared-origin proxy instead.
export const handleFetch: HandleFetch = async ({ event, request, fetch }) => {
  const url = new URL(request.url);
  if (url.origin === event.url.origin && url.pathname.startsWith('/api')) {
    const axum = env.AXUM_URL ?? 'http://127.0.0.1:3100';
    request = new Request(new URL(url.pathname + url.search, axum), request);
  }

  return fetch(request);
};

import type { MySkinr } from '#lib/types.js';
import type { PageServerLoad } from './$types';

// A logged-in character's SKINR, read from ESI by the API. Logged out (401),
// the page invites a login instead; any other failure is said on the page.
export const load: PageServerLoad = async ({ fetch }) => {
  const response = await fetch('/api/me/skinr').catch(() => null);
  if (response?.ok) return { mine: (await response.json()) as MySkinr, problem: null };
  if (response?.status === 401) return { mine: null, problem: null };
  const body: { message?: string } = (await response?.json().catch(() => ({}))) ?? {};
  return { mine: null, problem: body.message ?? 'The server is unavailable right now.' };
};

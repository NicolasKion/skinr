import { isLight, shipFileUrls } from '#lib/viewer/resources.js';

/**
 * The light game files a ship will need (its catalog and shaders, `isLight`),
 * for a page's head: the browser then starts on them with the HTML, long
 * before the engine can ask. Only a head start, so nothing when the API
 * cannot say.
 */
export async function filesAhead(
  fetch: typeof globalThis.fetch,
  id: number,
  dna: string,
): Promise<string[]> {
  try {
    const [build, listed] = await Promise.all([
      fetch('/cdn/build').then((response) => response.json() as Promise<{ build: string }>),
      fetch(`/api/ships/${id}/files`).then((response) => response.json() as Promise<string[]>),
    ]);
    return shipFileUrls(build.build, dna, listed).filter(isLight);
  } catch {
    return [];
  }
}

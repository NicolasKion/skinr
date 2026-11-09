// The game client's files and the Space Object Factory, read in the browser
// by path through the API (src/game_files): the API holds the client's file
// index, so the page downloads only the files a ship uses, never the 30 MB
// index, and gets them zstd-compressed where the browser takes it.
/* eslint-disable @typescript-eslint/no-explicit-any */
export type SpaceObjectFactory = any;

/** The scheme of a game file path. */
const RES_ROOT = 'res:/';

/** One client build's files, by `res:/` path. */
export interface GameFiles {
  build: string;
  /** Where the API serves a path. */
  url(path: string): string;
  /** A path's bytes; throws for a file the build does not have. */
  read(path: string): Promise<Uint8Array>;
  /** Whether the build has a path, from its folder's listing (fetched once per folder). */
  exists(path: string): Promise<boolean>;
}

/** The URL of a game file under `/cdn/res/{build}/`. */
export function resUrl(build: string, path: string): string {
  const relative = path.startsWith(RES_ROOT) ? path.slice(RES_ROOT.length) : path;
  const segments = relative.replaceAll('\\', '/').split('/').filter(Boolean);
  return `/cdn/res/${encodeURIComponent(build)}/${segments.map(encodeURIComponent).join('/')}`;
}

/** A path's folder and file name, lowercase, as the API's listing names them. */
export function splitPath(path: string): { folder: string; name: string } {
  const relative = (path.startsWith(RES_ROOT) ? path.slice(RES_ROOT.length) : path)
    .replaceAll('\\', '/')
    .toLowerCase();
  const at = relative.lastIndexOf('/');
  return { folder: relative.slice(0, Math.max(0, at)), name: relative.slice(at + 1) };
}

/** The files of one build, as the API names it. */
export function gameFiles(build: string, fetcher: typeof fetch = fetch): GameFiles {
  const listings = new Map<string, Promise<Set<string>>>();
  const listing = (folder: string) => {
    let names = listings.get(folder);
    if (!names) {
      names = fetcher(`/cdn/list/${encodeURIComponent(build)}/${folder}`)
        .then((response) => (response.ok ? response.json() : []))
        .then((list: string[]) => new Set(list));
      // A failed listing is asked for again next time rather than kept.
      names.catch(() => listings.delete(folder));
      listings.set(folder, names);
    }
    return names;
  };
  return {
    build,
    url: (path) => resUrl(build, path),
    async read(path) {
      const response = await fetcher(resUrl(build, path));
      if (!response.ok) throw new Error(`HTTP ${response.status} for ${path}`);
      return new Uint8Array(await response.arrayBuffer());
    },
    async exists(path) {
      const { folder, name } = splitPath(path);
      return (await listing(folder)).has(name);
    },
  };
}

let opened: Promise<GameFiles> | null = null;

/** The current client build's files, shared by the engine and preloading. */
export function openGameFiles(): Promise<GameFiles> {
  opened ??= fetch('/cdn/build')
    .then(async (response) => {
      if (!response.ok) throw new Error(`HTTP ${response.status} for the client build`);
      const { build } = (await response.json()) as { build: string };
      return gameFiles(build);
    })
    .catch((error) => {
      // Let the next caller try again rather than keeping the failure.
      opened = null;
      throw error;
    });
  return opened;
}

/** URLs already fetched ahead, so a ship picked twice is not fetched twice. */
const preloaded = new Set<string>();

/**
 * Fetches every game file a ship's render used (src/server/ship_files.rs),
 * all at once, into the browser's cache. The runtime would otherwise find
 * them one after another, hull first, then its textures and shaders; this
 * way they are on their way while the engine is still starting. The URLs are
 * immutable, so the runtime's own requests are then answered from the cache.
 */
export async function preloadShip(id: number) {
  try {
    const [files, response] = await Promise.all([openGameFiles(), fetch(`/api/ships/${id}/files`)]);
    if (!response.ok) return;
    const paths = (await response.json()) as string[];
    const urls = paths.map((path) => files.url(path)).filter((url) => !preloaded.has(url));
    for (const url of urls) preloaded.add(url);
    await Promise.all(
      urls.map((url) =>
        // Low priority: the engine's own requests, which it waits on, go first.
        fetch(url, { priority: 'low' })
          .then((file) => file.arrayBuffer())
          .catch(() => preloaded.delete(url)),
      ),
    );
  } catch {
    // Only a head start: the runtime loads whatever is not cached by itself.
  }
}

export async function openSof(files: GameFiles): Promise<SpaceObjectFactory> {
  // Loaded with the engine, not with this module (which the tests import).
  const { EveSOF } = await import('@carbonenginejs/runtime/sof');
  const sof = await new EveSOF().Register({
    lazyData: { source: (path: string) => fetch(files.url(path)) },
    resources: { exists: (path: string) => files.exists(path) },
  });
  await sof.InitializeAsync();
  return sof;
}

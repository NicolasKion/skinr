// The game client's files and the Space Object Factory, read in the browser
// by path through the API (src/game_files): the API holds the client's file
// index, so the page downloads only the files a ship uses, never the 30 MB
// index, and gets them zstd-compressed where the browser takes it.
/* eslint-disable @typescript-eslint/no-explicit-any */
export type SpaceObjectFactory = any;

import { isEffectSource } from './effects';
import { worthPreloading } from './unused';

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

/**
 * The URL of a game file under `/cdn/res/{build}/`. Lowercase, as the index
 * has every path: a file fetched ahead and the same file asked for by the
 * runtime, in whatever case it was authored, are then one cache entry.
 */
export function resUrl(build: string, path: string): string {
  const relative = path.toLowerCase().startsWith(RES_ROOT) ? path.slice(RES_ROOT.length) : path;
  const segments = relative.toLowerCase().replaceAll('\\', '/').split('/').filter(Boolean);
  return `/cdn/res/${encodeURIComponent(build)}/${segments.map(encodeURIComponent).join('/')}`;
}

/** The `res:/` path a URL from `resUrl` serves. */
export function resPath(url: string): string {
  const [, , , , ...segments] = url.split('/');
  return RES_ROOT + segments.map(decodeURIComponent).join('/');
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

/** Where the Space Object Factory keeps its catalog. */
const SOF_ROOT = 'res:/dx9/model/spaceobjectfactory';
/** The catalog folders a DNA's first three parts name a record in, in order. */
const SOF_FOLDERS = ['hulls', 'factions', 'races'];

/**
 * The catalog files the factory reads first for a DNA (`hull:faction:race`,
 * then commands): it only learns of them once it is running, one after
 * another. A record kept under another file name is simply not found ahead.
 */
export function sofFiles(dna: string): string[] {
  const names = dna.split('?')[0].split(':');
  return [
    `${SOF_ROOT}/generic.black`,
    ...SOF_FOLDERS.flatMap((folder, at) =>
      names[at] ? [`${SOF_ROOT}/${folder}/${names[at].toLowerCase()}.black`] : [],
    ),
  ];
}

/**
 * The URLs to fetch ahead for a ship: its DNA's catalog files, then what its
 * last render used (`/api/ships/{id}/files`), without what is never drawn.
 */
export function shipFileUrls(build: string, dna: string | null, listed: string[]): string[] {
  const paths = worthPreloading([...(dna ? sofFiles(dna) : []), ...listed]);
  return [...new Set(paths.map((path) => resUrl(build, path)))];
}

/**
 * Files with work waiting behind them once they land: the catalog (the ship
 * is built from it), graphs and shaders (translated in workers). Together a
 * few hundred kB of a ship's megabytes.
 */
const LIGHT_FILES = /\.(black|red|sm_depth)$/;

/**
 * Whether a file is fetched at once, even beside the engine's own code. The
 * rest (geometry, textures) is most of the bytes, and servers share a
 * connection evenly between everything on it: asked for at once, they would
 * hold the code back to the end, and nothing can start without it.
 */
export function isLight(url: string): boolean {
  return LIGHT_FILES.test(url);
}

/**
 * Heavy files under way while the engine's code is still coming: enough to
 * keep the line busy while the code is only being run, few enough to leave
 * it most of the line while it downloads.
 */
const HEAVY_BESIDE_CODE = 2;

/** Lets `limit` holders in at a time until it is opened, then everyone. */
export function createGate(limit: number) {
  const waiting: (() => void)[] = [];
  let inside = 0;
  let open = false;
  return {
    /** Resolves when it is the caller's turn, with the function to call when done. */
    enter(): Promise<() => void> {
      const leave = () => {
        inside -= 1;
        const next = waiting.shift();
        if (next) next();
      };
      return new Promise((resolve) => {
        const go = () => {
          inside += 1;
          resolve(leave);
        };
        if (open || inside < limit) go();
        else waiting.push(go);
      });
    },
    /** Lets in everyone waiting, and everyone after. */
    open() {
      open = true;
      for (const next of waiting.splice(0)) next();
    },
  };
}

/** What the heavy files pass through: narrow until the engine's code is in. */
const heavy = createGate(HEAVY_BESIDE_CODE);

/** Names the load of the engine's code; once it settles the heavy files all go. */
export function afterCode(loading: Promise<unknown>) {
  void loading.catch(() => {}).then(() => heavy.open());
}

/** URLs already fetched ahead, so a ship picked twice is not fetched twice. */
const preloaded = new Set<string>();

/**
 * Fetches every game file a ship's render used (src/server/ship_files.rs)
 * into the browser's cache. The runtime would otherwise find them one after
 * another, hull first, then its textures and shaders; this way they are on
 * their way while the engine is still starting: the light ones at once, the
 * heavy ones a few at a time until the engine's code is in (`isLight`). The URLs are
 * immutable, so the runtime's own requests are then answered from the cache.
 * A page that opens on a ship has the light ones in its head, where the
 * browser starts on them before any script runs; these fetches join those.
 */
export async function preloadShip(id: number, dna: string | null = null) {
  try {
    const [files, response] = await Promise.all([openGameFiles(), fetch(`/api/ships/${id}/files`)]);
    const listed = response.ok ? ((await response.json()) as string[]) : [];
    const urls = shipFileUrls(files.build, dna, listed).filter((url) => !preloaded.has(url));
    for (const url of urls) preloaded.add(url);
    await Promise.all(
      urls.map(async (url) => {
        const leave = isLight(url) ? null : await heavy.enter();
        try {
          // Low priority: the engine's own requests, which it waits on, go first.
          const bytes = await (await fetch(url, { priority: 'low' })).arrayBuffer();
          leave?.();
          // Shaders are translated while the engine is still starting (effects.ts).
          if (isEffectSource(url)) {
            const { translations } = await import('./effect-workers');
            void translations.prepare(resPath(url), new Uint8Array(bytes));
          }
        } catch {
          leave?.();
          preloaded.delete(url);
        }
      }),
    );
  } catch {
    // Only a head start: the runtime loads whatever is not cached by itself.
  }
}

/**
 * Opens the Space Object Factory. Its catalog files load through the
 * resource manager, so the runtime must have its file source by now
 * (backend.ts, `startRuntime`).
 */
export async function openSof(files: GameFiles): Promise<SpaceObjectFactory> {
  // Loaded with the engine, not with this module (which the tests import).
  const [{ EveSOF }, { blue }, { pumped }] = await Promise.all([
    import('@carbonenginejs/runtime/sof'),
    import('@carbonenginejs/runtime/blue'),
    import('./pump'),
  ]);
  const sof = await new EveSOF().Register({
    lazyData: { source: (path: string) => blue.resMan.LoadObject(path) },
    resources: { exists: (path: string) => files.exists(path) },
  });
  await pumped(sof.InitializeAsync());
  return sof;
}

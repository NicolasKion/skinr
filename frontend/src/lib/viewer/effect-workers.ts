// The page's effect translations (effects.ts): a few workers that translate,
// and IndexedDB to keep what they made for the next visit.
import { EffectTranslations, type Built, type TranslationStore } from './effects';
import type { TranslateAnswer, TranslateRequest } from './effects.worker';
import workerUrl from './effects.worker?worker&url';
import { openGameFiles } from './resources';

/** Workers translating at once, at most; one core is left to the page. */
const MAX_WORKERS = 4;
/** What a browser that does not say is taken to have. */
const ASSUMED_CORES = 4;

const DATABASE = 'effect-translations';
const BUILT = 'built';
const META = 'meta';
/** The record naming what the kept translations were made from. */
const MADE_FROM = 'made-from';
/** Between an effect's path and its permutation in a key (no path has one). */
const SEPARATOR = '|';
/** Sorts after every permutation key, to bound an effect's range. */
const RANGE_END = '￿';

/** A few workers, each translating one effect at a time; started on first use. */
function workerPool() {
  const size = Math.min(
    MAX_WORKERS,
    Math.max(1, (navigator.hardwareConcurrency || ASSUMED_CORES) - 1),
  );
  /** A worker and the one job it is on. */
  interface Seat {
    worker: Worker;
    job: { resolve: (built: Built) => void; reject: (error: Error) => void } | null;
  }
  const idle: Seat[] = [];
  let started = 0;
  const queue: ((seat: Seat) => void)[] = [];

  const start = () => {
    const seat: Seat = { worker: new Worker(workerUrl, { type: 'module' }), job: null };
    seat.worker.addEventListener('message', ({ data }: MessageEvent<TranslateAnswer>) => {
      if ('error' in data) seat.job?.reject(new Error(data.error));
      else seat.job?.resolve({ bytes: data.bytes, indices: data.indices });
    });
    seat.worker.addEventListener('error', (event) => seat.job?.reject(new Error(event.message)));
    return seat;
  };
  const take = () => {
    const free = idle.pop();
    if (free) return Promise.resolve(free);
    if (started < size) {
      started += 1;
      return Promise.resolve(start());
    }
    return new Promise<Seat>((resolve) => queue.push(resolve));
  };
  const give = (seat: Seat) => {
    seat.job = null;
    const next = queue.shift();
    if (next) next(seat);
    else idle.push(seat);
  };

  return async (path: string, bytes: Uint8Array): Promise<Built> => {
    const seat = await take();
    try {
      return await new Promise<Built>((resolve, reject) => {
        seat.job = { resolve, reject };
        // A copy: the caller's bytes go on to the runtime.
        const copy = bytes.slice();
        seat.worker.postMessage({ path, bytes: copy } satisfies TranslateRequest, [copy.buffer]);
      });
    } finally {
      give(seat);
    }
  };
}

function settled<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.addEventListener('success', () => resolve(request.result));
    request.addEventListener('error', () => reject(request.error));
  });
}

/**
 * Translations in IndexedDB. They hold for one client build and one
 * translator (the worker's hashed URL changes with its code); anything made
 * from another is dropped on opening.
 */
function indexedStore(): TranslationStore {
  const opening = (async () => {
    const { build } = await openGameFiles();
    const madeFrom = `${workerUrl}${SEPARATOR}${build}`;
    const request = indexedDB.open(DATABASE, 1);
    request.addEventListener('upgradeneeded', () => {
      request.result.createObjectStore(BUILT);
      request.result.createObjectStore(META);
    });
    const database = await settled(request);
    const meta = database.transaction([META, BUILT], 'readwrite');
    if ((await settled(meta.objectStore(META).get(MADE_FROM))) !== madeFrom) {
      meta.objectStore(BUILT).clear();
      meta.objectStore(META).put(madeFrom, MADE_FROM);
    }
    return database;
  })();
  // Without a database (private windows, a full disk) nothing is kept.
  opening.catch(() => {});

  return {
    async load(path) {
      const kept = new Map<string, Built>();
      try {
        const prefix = path + SEPARATOR;
        const range = IDBKeyRange.bound(prefix, prefix + RANGE_END);
        const built = (await opening).transaction(BUILT).objectStore(BUILT);
        const [keys, values] = await Promise.all([
          settled(built.getAllKeys(range)),
          settled(built.getAll(range)),
        ]);
        keys.forEach((key, at) => kept.set(String(key).slice(prefix.length), values[at]));
      } catch {
        // Nothing kept: the effect is translated again.
      }
      return kept;
    },
    save(path, permutation, built) {
      opening
        .then((database) => {
          const store = database.transaction(BUILT, 'readwrite').objectStore(BUILT);
          store.put(built, path + SEPARATOR + permutation);
        })
        .catch(() => {});
    },
  };
}

/** Kept between visits only in a build: a dev worker's URL never changes with its code. */
export const translations = new EffectTranslations(
  workerPool(),
  import.meta.env.DEV ? null : indexedStore(),
);

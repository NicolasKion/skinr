// EVE's shaders ship as dx11 effect containers, and the runtime translates
// each to WebGPU on the main thread as it loads: a second on a fast desktop,
// several on a laptop, all of it while the page is trying to come up. Here
// the same translation is done ahead: in workers as soon as a ship's files
// are known, and from a store on a later visit. The runtime's translator is
// wrapped so it finds the result instead of working it out.
//
// No engine imports: the translator, the workers and the store are handed in
// (effect-workers.ts), so the tests cover this as it is.

/** What the runtime needs of one translated permutation (Tr2EffectRes, BuiltPermutation). */
export interface Built {
  /** The translated container. */
  bytes: Uint8Array;
  /** Every permutation index that shares this body. */
  indices: number[];
}

/** Translations kept between visits. Neither call may reject. */
export interface TranslationStore {
  /** Every permutation kept for an effect, by `permutationKey`. */
  load(path: string): Promise<Map<string, Built>>;
  save(path: string, permutation: string, built: Built): void;
}

/** The runtime's translator, as far as Tr2EffectRes uses it. */
export interface Translator {
  buildEffect(data: Uint8Array | ArrayBuffer, options: BuildOptions): any;
}

interface BuildOptions {
  source?: string;
  permutation?: unknown;
  mode?: string;
}

/* eslint-disable @typescript-eslint/no-explicit-any */

/** The tree the client's effects are read from; only these are translated. */
const SOURCE_TREE = '/effect.dx11/';

/** The permutation an effect loads with: no option chosen. */
const DEFAULT_PERMUTATION: unknown[] = [];

/** Whether `path` is an effect the runtime translates when it loads. */
export function isEffectSource(path: string): boolean {
  return path.toLowerCase().includes(SOURCE_TREE);
}

/** How a permutation is named in the store and in memory. */
export function permutationKey(permutation: unknown): string {
  return JSON.stringify(permutation ?? DEFAULT_PERMUTATION);
}

/** A translator's full result cut down to what is kept. */
export function reduce(built: any): Built {
  const { variants } = built.permutationGraph;
  const body = variants.find(
    (variant: any) => variant.permutationIndex === built.metadata.bodyIndex,
  )?.bodyKey;
  return {
    bytes: built.bytes,
    indices: variants
      .filter((variant: any) => variant.bodyKey === body)
      .map((variant: any) => variant.permutationIndex),
  };
}

/** A kept result in the shape the runtime reads: one body, shared by `indices`. */
function expand(built: Built) {
  return {
    bytes: built.bytes,
    metadata: { bodyIndex: built.indices[0] },
    permutationGraph: {
      variants: built.indices.map((permutationIndex) => ({ permutationIndex, bodyKey: 0 })),
    },
  };
}

export class EffectTranslations {
  /** Translated and not yet asked for by the runtime, by path then permutation. */
  private ready = new Map<string, Map<string, Built>>();
  /** Effects being prepared, or prepared already. */
  private prepared = new Map<string, Promise<void>>();
  private translateAway: (path: string, bytes: Uint8Array) => Promise<Built>;
  private store: TranslationStore | null;

  /**
   * `translateAway` translates an effect's default permutation off the main
   * thread; `store` keeps results between visits.
   */
  constructor(
    translateAway: (path: string, bytes: Uint8Array) => Promise<Built>,
    store: TranslationStore | null = null,
  ) {
    this.translateAway = translateAway;
    this.store = store;
  }

  /**
   * Has an effect's translation waiting before the runtime loads it: from
   * the store, with every permutation an earlier visit used, or else from a
   * worker. Asked twice, an effect is prepared once. Never rejects: an
   * effect that could not be prepared is translated by the runtime itself.
   */
  prepare(path: string, bytes: Uint8Array): Promise<void> {
    const key = path.toLowerCase();
    let work = this.prepared.get(key);
    if (!work) {
      work = this.fill(key, bytes).catch(() => {});
      this.prepared.set(key, work);
    }
    return work;
  }

  private async fill(key: string, bytes: Uint8Array) {
    const kept = (await this.store?.load(key)) ?? new Map<string, Built>();
    const standard = permutationKey(DEFAULT_PERMUTATION);
    if (!kept.has(standard)) {
      const built = await this.translateAway(key, bytes);
      kept.set(standard, built);
      this.store?.save(key, standard, built);
    }
    this.ready.set(key, kept);
  }

  /**
   * `translator` with every prepared result answered from memory. What was
   * not prepared (a permutation picked while drawing) is translated as
   * before, and kept for the next visit.
   */
  wrap(translator: Translator): Translator {
    return {
      buildEffect: (data, options) => {
        // Only one permutation at a time is kept; anything else goes through.
        if (!options.source || options.mode !== undefined) {
          return translator.buildEffect(data, options);
        }
        const key = options.source.toLowerCase();
        const permutation = permutationKey(options.permutation);
        const waiting = this.ready.get(key);
        const built = waiting?.get(permutation);
        if (built) {
          // The runtime holds it from here; a second copy would only cost memory.
          waiting!.delete(permutation);
          return expand(built);
        }
        const translated = translator.buildEffect(data, options);
        this.store?.save(key, permutation, reduce(translated));
        return translated;
      },
    };
  }
}

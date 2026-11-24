import { describe, expect, it, vi } from 'vitest';
import {
  EffectTranslations,
  isEffectSource,
  permutationKey,
  reduce,
  type Built,
  type TranslationStore,
} from './effects';

const PATH = 'res:/graphics/effect.dx11/managed/space/hull.sm_depth';
const SOURCE = new Uint8Array([1, 2, 3]);

/** A translator's result with two bodies: permutations 0 and 2 share the first. */
function result(tag: number, bodyIndex = 0) {
  return {
    bytes: new Uint8Array([tag]),
    metadata: { bodyIndex },
    permutationGraph: {
      variants: [
        { permutationIndex: 0, bodyKey: 'a' },
        { permutationIndex: 1, bodyKey: 'b' },
        { permutationIndex: 2, bodyKey: 'a' },
      ],
    },
  };
}

/** What Tr2EffectRes reads out of a translator's result. */
function read(built: ReturnType<typeof result>) {
  const { bytes, indices } = reduce(built);
  return { bytes: [...bytes], indices };
}

function memoryStore(kept = new Map<string, Map<string, Built>>()) {
  const store: TranslationStore = {
    load: vi.fn(async (path: string) => new Map(kept.get(path))),
    save: vi.fn((path: string, permutation: string, built: Built) => {
      if (!kept.has(path)) kept.set(path, new Map());
      kept.get(path)!.set(permutation, built);
    }),
  };
  return { store, kept };
}

describe('isEffectSource', () => {
  it('knows the dx11 tree from everything else', () => {
    expect(isEffectSource('res:/Graphics/Effect.dx11/managed/hull.sm_depth')).toBe(true);
    expect(isEffectSource('res:/graphics/effect.webgpu/managed/hull.sm_depth')).toBe(false);
    expect(isEffectSource('res:/dx9/model/ship/hull.gr2')).toBe(false);
  });
});

describe('reduce', () => {
  it('keeps the bytes and the permutations sharing the body', () => {
    expect(read(result(7))).toEqual({ bytes: [7], indices: [0, 2] });
    expect(read(result(7, 1))).toEqual({ bytes: [7], indices: [1] });
  });
});

describe('EffectTranslations', () => {
  it('answers a prepared effect without translating on the main thread', async () => {
    const away = vi.fn(async () => reduce(result(7)));
    const translator = { buildEffect: vi.fn(() => result(9)) };
    const translations = new EffectTranslations(away);

    await translations.prepare(PATH, SOURCE);
    const built = translations.wrap(translator).buildEffect(SOURCE, {
      source: PATH,
      permutation: [],
    });

    expect(read(built)).toEqual({ bytes: [7], indices: [0, 2] });
    expect(translator.buildEffect).not.toHaveBeenCalled();
  });

  it('prepares an effect once, whatever the case of its path', async () => {
    const away = vi.fn(async () => reduce(result(7)));
    const translations = new EffectTranslations(away);

    await Promise.all([
      translations.prepare(PATH, SOURCE),
      translations.prepare(PATH.toUpperCase(), SOURCE),
    ]);

    expect(away).toHaveBeenCalledTimes(1);
  });

  it('lets the runtime translate what a worker could not', async () => {
    const translator = { buildEffect: vi.fn(() => result(9)) };
    const translations = new EffectTranslations(async () => {
      throw new Error('no workers');
    });

    await translations.prepare(PATH, SOURCE);
    const built = translations.wrap(translator).buildEffect(SOURCE, {
      source: PATH,
      permutation: [],
    });

    expect(read(built)).toEqual({ bytes: [9], indices: [0, 2] });
  });

  it('keeps a permutation picked while drawing for the next visit', async () => {
    const { store, kept } = memoryStore();
    const permutation = [{ name: 'skinned', value: 'on' }];
    const first = new EffectTranslations(async () => reduce(result(7)), store);
    await first.prepare(PATH, SOURCE);
    first.wrap({ buildEffect: () => result(9, 1) }).buildEffect(SOURCE, {
      source: PATH,
      permutation,
    });
    expect([...kept.get(PATH)!.keys()]).toEqual([permutationKey([]), permutationKey(permutation)]);

    const away = vi.fn(async () => reduce(result(7)));
    const translator = { buildEffect: vi.fn(() => result(0)) };
    const next = new EffectTranslations(away, store);
    await next.prepare(PATH, SOURCE);
    const wrapped = next.wrap(translator);

    expect(read(wrapped.buildEffect(SOURCE, { source: PATH, permutation: [] }))).toEqual({
      bytes: [7],
      indices: [0, 2],
    });
    expect(read(wrapped.buildEffect(SOURCE, { source: PATH, permutation }))).toEqual({
      bytes: [9],
      indices: [1],
    });
    expect(away).not.toHaveBeenCalled();
    expect(translator.buildEffect).not.toHaveBeenCalled();
  });

  it('passes a request for every permutation straight through', () => {
    const translator = { buildEffect: vi.fn(() => result(9)) };
    const translations = new EffectTranslations(async () => reduce(result(7)));

    translations.wrap(translator).buildEffect(SOURCE, { source: PATH, mode: 'all' });

    expect(translator.buildEffect).toHaveBeenCalledWith(SOURCE, { source: PATH, mode: 'all' });
  });
});

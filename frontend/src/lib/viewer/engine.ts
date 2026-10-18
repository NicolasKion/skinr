// One engine per page load: the file index, the Space Object Factory and the
// WebGPU renderer are expensive to set up and the runtime keeps global state,
// so every viewport shares them.
import { openLibrary, openSof } from './resources';
import type { ScreenRect } from './poses';
import type { Blueprint, ShipRenderer } from './renderer';

export type { Blueprint };

export type EngineStage = 'index' | 'sof' | 'gpu' | 'ready';

/**
 * A new ship flies in; a new livery of the same ship (or any new ship, where
 * a page asks) slides in from a side, calling `onGlide` as it starts moving; a
 * ship opened from its card is pinned over the card's thumbnail at `rect`
 * (renderer.pinShip), to be released from there.
 */
export type ShowTransition =
  | { kind: 'fly' }
  | { kind: 'slide'; direction: 1 | -1; onGlide?: () => void }
  | { kind: 'pin'; rect: ScreenRect };

export interface Engine {
  build: string;
  renderer: ShipRenderer;
  /**
   * Builds a ship from its SOF DNA and shows it, resolving once it has loaded
   * and is on its way in. Resolves false when a later call overtook it (only
   * the newest request ever reaches the screen).
   */
  show(dna: string, transition?: ShowTransition): Promise<boolean>;
  /**
   * Makes a pattern the SOF does not ship (a SKINR design's projections)
   * available to DNA under `name`, as a raw EveSOFDataPattern record.
   */
  registerPattern(name: string, record: object): void;
}

let engine: Promise<Engine> | null = null;

/** Where dev builds put the engine for poking at from the console or a test script. */
const DEBUG_HANDLE = '__shipEngine';

/** Everyone following the start, and how far it has got. */
const watchers = new Set<(stage: EngineStage) => void>();
let reached: EngineStage | null = null;
function report(stage: EngineStage) {
  reached = stage;
  for (const watcher of watchers) watcher(stage);
}

/**
 * The page's one engine, started on first use. `onStage` follows the start
 * whoever asked first, told at once how far it has got.
 */
export function getEngine(onStage?: (stage: EngineStage) => void): Promise<Engine> {
  if (onStage) {
    watchers.add(onStage);
    if (reached) onStage(reached);
  }
  engine ??= start().catch((error) => {
    // Let the next viewport try again rather than caching the failure.
    engine = null;
    throw error;
  });
  return engine;
}

async function start(): Promise<Engine> {
  report('index');
  const library = await openLibrary();

  report('sof');
  const sof = await openSof(library);

  report('gpu');
  // Loaded only in the browser, and only once a viewport asks for it.
  const { ShipRenderer } = await import('./renderer');
  const renderer = await ShipRenderer.create(library);

  report('ready');
  let latest = 0;
  /** The DNA last handed to the renderer: on screen, or queued to slide in. */
  let target: string | null = null;
  const ready: Engine = {
    build: String(library.build),
    renderer,
    registerPattern(name: string, record: object) {
      sof.dataMgr.UpdatePattern(name, record);
    },
    async show(dna: string, transition: ShowTransition = { kind: 'fly' }) {
      const request = ++latest;
      if (dna === target) return true;
      const values = await sof.BuildValuesFromDNAAsync(dna);
      if (request !== latest) return false;
      target = dna;
      if (transition.kind === 'slide') {
        return renderer.slideToShip(values, transition.direction, transition.onGlide);
      }
      if (transition.kind === 'pin') return renderer.pinShip(values, transition.rect);
      return renderer.setShip(values);
    },
  };
  // For poking at the live scene from the console or a test script while
  // developing; production builds drop it.
  if (import.meta.env.DEV) Object.assign(globalThis, { [DEBUG_HANDLE]: ready });
  return ready;
}

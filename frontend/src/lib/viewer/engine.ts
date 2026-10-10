// One engine per page load: the file index, the Space Object Factory and the
// WebGPU renderer are expensive to set up and the runtime keeps global state,
// so every viewport shares them.
import { openLibrary, openSof } from './resources';
import type { Blueprint, ShipRenderer } from './renderer';

export type { Blueprint };

export type EngineStage = 'index' | 'sof' | 'gpu' | 'ready';

/** A new ship flies in; a new livery of the same ship slides in from a side. */
export type ShowTransition = { kind: 'fly' } | { kind: 'slide'; direction: 1 | -1 };

export interface Engine {
  build: string;
  renderer: ShipRenderer;
  /**
   * Builds a ship from its SOF DNA and shows it. Resolves false when a later
   * call overtook it (only the newest request ever reaches the screen).
   */
  show(dna: string, transition?: ShowTransition): Promise<boolean>;
}

let engine: Promise<Engine> | null = null;

/** Where dev builds put the engine for poking at from the console or a test script. */
const DEBUG_HANDLE = '__shipEngine';

export function getEngine(onStage?: (stage: EngineStage) => void): Promise<Engine> {
  engine ??= start(onStage).catch((error) => {
    // Let the next viewport try again rather than caching the failure.
    engine = null;
    throw error;
  });
  return engine;
}

async function start(onStage?: (stage: EngineStage) => void): Promise<Engine> {
  onStage?.('index');
  const library = await openLibrary();

  onStage?.('sof');
  const sof = await openSof(library);

  onStage?.('gpu');
  // Loaded only in the browser, and only once a viewport asks for it.
  const { ShipRenderer } = await import('./renderer');
  const renderer = await ShipRenderer.create(library);

  onStage?.('ready');
  let latest = 0;
  /** The DNA last handed to the renderer: on screen, or queued to slide in. */
  let target: string | null = null;
  const ready: Engine = {
    build: String(library.build),
    renderer,
    async show(dna: string, transition: ShowTransition = { kind: 'fly' }) {
      const request = ++latest;
      if (dna === target) return true;
      const values = await sof.BuildValuesFromDNAAsync(dna);
      if (request !== latest) return false;
      target = dna;
      if (transition.kind === 'slide') return renderer.slideToShip(values, transition.direction);
      renderer.setShip(values);
      return true;
    },
  };
  // For poking at the live scene from the console or a test script while
  // developing; production builds drop it.
  if (import.meta.env.DEV) Object.assign(globalThis, { [DEBUG_HANDLE]: ready });
  return ready;
}

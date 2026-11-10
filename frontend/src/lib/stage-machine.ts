// Where the shared stage is, as one state machine. stage.svelte feeds it every
// frame (where the page wants the stage, whether that place is on the page,
// whether the ship has loaded) and places the canvas by the state it is in.
// Every change of place goes through `step`, so what the stage does between
// two pages is decided here, not by the order pages mount and unmount in.

/** The places a page can put the stage (`Scene.place`). */
export type Place = 'hero' | 'peek' | 'blueprint';

export type Presence<G> =
  /** Out of sight: no page wants it, or its place is not on the page. */
  | { kind: 'hidden' }
  /** Wanted, but kept out of sight until its ship has loaded. */
  | { kind: 'appearing'; place: Place }
  /**
   * On its place, following it (and the scroll). `fresh` on the first frame
   * after it came into sight, which it takes without easing.
   */
  | { kind: 'shown'; place: Place; fresh: boolean }
  /** On its way from where it was (`from`, on screen) to a place, since `start`. */
  | { kind: 'gliding'; place: Place; from: G; start: number }
  /**
   * Wanted somewhere that is not on the page yet (the next page's dock, still
   * being laid out): it stays where it is for up to HOLD_MS, then leaves.
   */
  | { kind: 'holding'; place: Place; since: number }
  /** Fading out where it is, since `since`. */
  | { kind: 'leaving'; since: number }
  /** Covering the viewport for a leap to another ship (#lib/leap.svelte.ts). */
  | { kind: 'leaping' };

export interface StageInput<G> {
  /** The frame's time, in ms. */
  now: number;
  /** Where the page wants the stage; null when no page does. */
  want: Place | null;
  /** Whether that place is on the page: the hero always is, a dock once measured. */
  found: boolean;
  /** Whether the ship asked for has loaded (or failed to), or none is asked for. */
  loaded: boolean;
  /** Whether a leap is under way. */
  leaping: boolean;
  /** The geometry on screen now, in viewport terms: where a glide starts. */
  current: G;
}

/** How long the stage waits in place for its next place to be laid out, in ms. */
export const HOLD_MS = 200;
/** How long fading out takes (stage.svelte's `.gone`), in ms. */
export const LEAVE_MS = 250;

/** Whether the stage is on screen in this state (a leap's ship shows once loaded). */
export function onScreen(state: Presence<unknown>): boolean {
  return state.kind === 'shown' || state.kind === 'gliding' || state.kind === 'holding';
}

/**
 * The state after one frame. Returns `state` itself when nothing changes, so
 * a caller can tell a transition by identity. `glideMs` is how long a glide
 * between two places takes.
 */
export function step<G>(state: Presence<G>, input: StageInput<G>, glideMs: number): Presence<G> {
  const { now, want, found, loaded, current } = input;
  if (input.leaping) return state.kind === 'leaping' ? state : { kind: 'leaping' };

  switch (state.kind) {
    case 'leaping':
      // A leap ends on the new page with the ship already where it belongs.
      return want && found ? { kind: 'shown', place: want, fresh: false } : { kind: 'hidden' };

    case 'hidden':
    case 'leaving':
    case 'appearing':
      // Out of sight, the stage takes its place at once; it shows only with
      // its ship loaded, never a half-streamed hull or the previous one.
      if (want && found) {
        if (loaded) return { kind: 'shown', place: want, fresh: true };
        return state.kind === 'appearing' && state.place === want
          ? state
          : { kind: 'appearing', place: want };
      }
      if (state.kind === 'leaving') {
        return now - state.since >= LEAVE_MS ? { kind: 'hidden' } : state;
      }
      return state.kind === 'hidden' ? state : { kind: 'hidden' };

    case 'shown':
    case 'gliding':
    case 'holding': {
      if (!want) return { kind: 'leaving', since: now };
      if (!found) {
        if (state.kind !== 'holding') return { kind: 'holding', place: want, since: now };
        if (now - state.since >= HOLD_MS) return { kind: 'leaving', since: now };
        return state.place === want ? state : { ...state, place: want };
      }
      // On screen, every change of place glides, from wherever the stage is.
      if (state.kind === 'holding' || state.place !== want) {
        return { kind: 'gliding', place: want, from: current, start: now };
      }
      if (state.kind === 'gliding') {
        return now - state.start >= glideMs ? { kind: 'shown', place: want, fresh: false } : state;
      }
      return state.fresh ? { ...state, fresh: false } : state;
    }
  }
}

/** One line for the trace: the state's kind and place. */
export function describe(state: Presence<unknown>): string {
  return 'place' in state ? `${state.kind}(${state.place})` : state.kind;
}

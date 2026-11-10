import { describe, expect, it } from 'vitest';
import { HOLD_MS, LEAVE_MS, step, type Presence, type StageInput } from './stage-machine';

/** How long a glide takes in these tests, in ms. */
const GLIDE = 900;

type State = Presence<string>;

const input = (overrides: Partial<StageInput<string>>): StageInput<string> => ({
  now: 0,
  want: 'hero',
  found: true,
  loaded: true,
  leaping: false,
  current: 'here',
  ...overrides,
});

/** Runs the machine through a list of frames, returning every state. */
function run(start: State, frames: Partial<StageInput<string>>[]): State[] {
  const states: State[] = [];
  let state = start;
  for (const frame of frames) {
    state = step(state, input(frame), GLIDE);
    states.push(state);
  }
  return states;
}

describe('step', () => {
  it('shows a wanted stage at once once its ship has loaded', () => {
    expect(step({ kind: 'hidden' }, input({}), GLIDE)).toEqual({
      kind: 'shown',
      place: 'hero',
      fresh: true,
    });
  });

  it('keeps a stage out of sight until its ship has loaded', () => {
    const [waiting, shown] = run({ kind: 'hidden' }, [
      { loaded: false },
      { loaded: true, now: 500 },
    ]);
    expect(waiting).toEqual({ kind: 'appearing', place: 'hero' });
    expect(shown).toMatchObject({ kind: 'shown', fresh: true });
  });

  it('eases again after the first frame in sight', () => {
    const shown: State = { kind: 'shown', place: 'peek', fresh: true };
    expect(step(shown, input({ want: 'peek' }), GLIDE)).toEqual({ ...shown, fresh: false });
  });

  it('returns the same state when nothing changes', () => {
    const shown: State = { kind: 'shown', place: 'hero', fresh: false };
    expect(step(shown, input({}), GLIDE)).toBe(shown);
  });

  it('glides between any two places on screen, from where it is', () => {
    const shown: State = { kind: 'shown', place: 'peek', fresh: false };
    const [gliding, still, landed] = run(shown, [
      { want: 'hero', now: 1000, current: 'in the panel' },
      { want: 'hero', now: 1000 + GLIDE / 2 },
      { want: 'hero', now: 1000 + GLIDE },
    ]);
    expect(gliding).toEqual({ kind: 'gliding', place: 'hero', from: 'in the panel', start: 1000 });
    expect(still).toBe(gliding);
    expect(landed).toEqual({ kind: 'shown', place: 'hero', fresh: false });
  });

  it('glides from dock to dock too', () => {
    const shown: State = { kind: 'shown', place: 'blueprint', fresh: false };
    expect(step(shown, input({ want: 'peek' }), GLIDE)).toMatchObject({
      kind: 'gliding',
      place: 'peek',
    });
  });

  it('turns a glide toward a new place from where it has got to', () => {
    const gliding: State = { kind: 'gliding', place: 'hero', from: 'panel', start: 0 };
    expect(
      step(gliding, input({ want: 'blueprint', now: 300, current: 'halfway' }), GLIDE),
    ).toEqual({ kind: 'gliding', place: 'blueprint', from: 'halfway', start: 300 });
  });

  it('holds still while the next page lays out its dock, then glides there', () => {
    const shown: State = { kind: 'shown', place: 'hero', fresh: false };
    const [holding, gliding] = run(shown, [
      { want: 'peek', found: false, now: 0 },
      { want: 'peek', found: true, now: 50, current: 'where it held' },
    ]);
    expect(holding).toEqual({ kind: 'holding', place: 'peek', since: 0 });
    expect(gliding).toEqual({ kind: 'gliding', place: 'peek', from: 'where it held', start: 50 });
  });

  it('leaves when a dock never turns up (a phone has no panel)', () => {
    const shown: State = { kind: 'shown', place: 'hero', fresh: false };
    const [, held, leaving, hidden] = run(shown, [
      { want: 'peek', found: false, now: 0 },
      { want: 'peek', found: false, now: HOLD_MS - 1 },
      { want: 'peek', found: false, now: HOLD_MS },
      { want: 'peek', found: false, now: HOLD_MS + LEAVE_MS },
    ]);
    expect(held.kind).toBe('holding');
    expect(leaving).toEqual({ kind: 'leaving', since: HOLD_MS });
    expect(hidden).toEqual({ kind: 'hidden' });
  });

  it('fades out when no page wants it', () => {
    const shown: State = { kind: 'shown', place: 'peek', fresh: false };
    expect(step(shown, input({ want: null, now: 10 }), GLIDE)).toEqual({
      kind: 'leaving',
      since: 10,
    });
  });

  it('comes straight back in place when wanted again while fading', () => {
    const leaving: State = { kind: 'leaving', since: 0 };
    expect(step(leaving, input({ want: 'peek', now: 100 }), GLIDE)).toEqual({
      kind: 'shown',
      place: 'peek',
      fresh: true,
    });
  });

  it('covers the viewport for a leap, whatever the page wants', () => {
    const shown: State = { kind: 'shown', place: 'hero', fresh: false };
    expect(step(shown, input({ leaping: true, want: 'blueprint' }), GLIDE)).toEqual({
      kind: 'leaping',
    });
  });

  it('lands a leap on the new page without a glide', () => {
    expect(step({ kind: 'leaping' }, input({}), GLIDE)).toEqual({
      kind: 'shown',
      place: 'hero',
      fresh: false,
    });
  });
});

// Frame-rate independent easing toward a moving target.

/** Closer than this, an eased value lands on its target. */
const LANDED = 1e-3;

/**
 * `value` moved toward `target` for `seconds`, covering a fixed share of what
 * is left per second (`rate`), and landing exactly once it is close.
 */
export function approach(value: number, target: number, rate: number, seconds: number): number {
  const next = value + (target - value) * (1 - Math.exp(-seconds * rate));
  return Math.abs(target - next) < LANDED ? target : next;
}

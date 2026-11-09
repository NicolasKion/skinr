import { describe, expect, it } from 'vitest';
import { PEEK_DEFAULT, parsePeek, peekWidth } from './peek';

describe('parsePeek', () => {
  it('reads a share of the viewport', () => {
    expect(parsePeek('0.42')).toBe(0.42);
  });

  it('falls back to the default for anything else', () => {
    for (const value of [undefined, null, '', 'wide', '0', '1', '-0.3', '1.5', 'NaN']) {
      expect(parsePeek(value)).toBeNull();
    }
  });
});

describe('peekWidth', () => {
  it('keeps the default until the visitor drags', () => {
    expect(peekWidth(null)).toBe(PEEK_DEFAULT);
  });

  it('clamps a dragged share to the bounds', () => {
    expect(peekWidth(0.42)).toBe('clamp(22rem, 42vw, 65vw)');
    expect(peekWidth(1 / 3)).toBe('clamp(22rem, 33.33vw, 65vw)');
  });
});

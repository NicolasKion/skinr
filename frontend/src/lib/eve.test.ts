import { describe, expect, it } from 'vitest';
import {
  DEFAULT_TINT,
  firstSentence,
  formatCompact,
  formatUnit,
  raceTint,
  typeIcon,
  typeRender,
} from './eve';

describe('eve', () => {
  it('builds image server URLs', () => {
    expect(typeIcon(603)).toBe('https://images.evetech.net/types/603/icon?size=64');
    expect(typeIcon(603, 32)).toBe('https://images.evetech.net/types/603/icon?size=32');
    expect(typeRender(638)).toBe('https://images.evetech.net/types/638/render?size=512');
  });

  it('formats figures with their unit', () => {
    expect(formatUnit(997000, 'kg')).toBe('997,000 kg');
    expect(formatUnit(16.25, 'm³')).toBe('16.3 m³');
    expect(formatUnit(null, 'm')).toBe('-');
  });

  it('formats large figures compactly', () => {
    expect(formatCompact(99300000)).toBe('99.3M');
    expect(formatCompact(470000)).toBe('470K');
    expect(formatCompact(150)).toBe('150');
    expect(formatCompact(null)).toBe('-');
  });

  it('tints by race, silver otherwise', () => {
    expect(raceTint(4)).toBe('#e8c170');
    expect(raceTint(null)).toBe(DEFAULT_TINT);
    expect(raceTint(999)).toBe(DEFAULT_TINT);
  });

  it('takes the first sentence', () => {
    expect(firstSentence('The Merlin is fast. It also shoots.')).toBe('The Merlin is fast.');
    expect(firstSentence('No full stop')).toBe('No full stop');
  });
});

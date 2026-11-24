import { describe, expect, it } from 'vitest';
import type { ShipSummary } from '#lib/types.js';
import { pickFeatured } from './featured';

const ship = (name: string) => ({ name }) as ShipSummary;

describe('pickFeatured', () => {
  const ships = [ship('Rifter'), ship('Raven'), ship('Astero')];

  it('picks among the featured ships by chance', () => {
    expect(pickFeatured(ships, 0)?.name).toBe('Raven');
    expect(pickFeatured(ships, 0.99)?.name).toBe('Astero');
  });

  it('falls back to the first ship, or none', () => {
    expect(pickFeatured([ship('Rifter')], 0.5)?.name).toBe('Rifter');
    expect(pickFeatured([], 0.5)).toBeUndefined();
  });
});

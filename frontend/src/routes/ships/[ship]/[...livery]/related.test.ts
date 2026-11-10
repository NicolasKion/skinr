import { describe, expect, it } from 'vitest';
import type { ShipDetail, ShipSummary } from '#lib/types.js';
import { relatedRails, summaryOf } from './related';

const colors = { primary: null, secondary: null, hull: null };

function summary(id: number, name: string, over: Partial<ShipSummary> = {}): ShipSummary {
  return {
    id,
    name,
    group_id: 25,
    race_id: 2,
    faction_id: 500002,
    meta_group_id: null,
    tech_level: 1,
    dna: 'mf4_t1:minmatarbase:minmatar',
    colors,
    ...over,
  };
}

const rifter = {
  id: 587,
  name: 'Rifter',
  group: { id: 25, name: 'Frigate' },
  race: { id: 2, name: 'Minmatar' },
  faction: { id: 500002, name: 'Minmatar Republic' },
  meta_group: null,
  tech_level: 1,
  dna: 'mf4_t1:minmatarbase:minmatar',
  colors,
} as unknown as ShipDetail;

const catalog = [
  summary(587, 'Rifter'),
  summary(11371, 'Wolf', { tech_level: 2, dna: 'mf4_t2a:minmatarbase:minmatar' }),
  summary(11400, 'Jaguar', { tech_level: 2, dna: 'mf4_t2b:minmatarbase:minmatar' }),
  summary(585, 'Slasher', { dna: 'mf1_t1:minmatarbase:minmatar' }),
  summary(603, 'Merlin', { race_id: 1, faction_id: 500001, dna: 'cf2_t1:caldaribase:caldari' }),
  summary(24702, 'Hurricane', { group_id: 419, dna: 'mbc1_t1:minmatarbase:minmatar' }),
  summary(17812, 'Republic Fleet Firetail', {
    group_id: 25,
    faction_id: 500002,
    dna: 'mf7_t1:minmatarnavy:minmatar',
  }),
];

describe('relatedRails', () => {
  it('lists the same frame, then the class, then the builder, each ship once', () => {
    const rails = relatedRails(rifter, catalog);
    expect(rails.map((rail) => rail.title)).toEqual([
      'Same frame',
      'More frigates',
      'More from Minmatar Republic',
    ]);
    expect(rails[0].ships.map((ship) => ship.name)).toEqual(['Jaguar', 'Wolf']);
    expect(rails[1].ships.map((ship) => ship.name)).toEqual([
      'Merlin',
      'Republic Fleet Firetail',
      'Slasher',
    ]);
    expect(rails[2].ships.map((ship) => ship.name)).toEqual(['Hurricane']);
    expect(rails[1].showClass).toBe(false);
  });

  it('never lists the ship itself, and drops empty rails', () => {
    const rails = relatedRails(rifter, [
      summary(587, 'Rifter'),
      summary(585, 'Slasher', { dna: 'mf1_t1:minmatarbase:minmatar' }),
    ]);
    expect(rails.map((rail) => rail.title)).toEqual(['More frigates']);
    expect(rails[0].ships.map((ship) => ship.id)).toEqual([585]);
  });

  it('falls back to the race for a ship without a faction', () => {
    const rails = relatedRails({ ...rifter, faction: null } as ShipDetail, [
      summary(24702, 'Hurricane', { group_id: 419, dna: 'mbc1_t1:minmatarbase:minmatar' }),
    ]);
    expect(rails.map((rail) => rail.title)).toEqual(['More Minmatar ships']);
  });
});

describe('summaryOf', () => {
  it('flattens a ship page’s ship into a catalog entry', () => {
    expect(summaryOf(rifter)).toEqual(summary(587, 'Rifter'));
  });
});

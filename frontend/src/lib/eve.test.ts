import { describe, expect, it } from 'vitest';
import type { Catalog, ShipSummary } from './types';
import {
  DEFAULT_TINT,
  shipBuilders,
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

describe('shipBuilders', () => {
  const GALLENTE = 8;
  const PIRATE = 32;
  const FEDERATION = 500004;
  const SISTERS = 500016;
  const GURISTAS = 500010;
  const ship = (id: number, race_id: number | null, faction_id: number | null): ShipSummary => ({
    id,
    name: `Ship ${id}`,
    group_id: 25,
    race_id,
    faction_id,
    meta_group_id: null,
    tech_level: 1,
    dna: '',
    colors: { primary: null, secondary: null, hull: null },
  });
  const catalog = (ships: ShipSummary[]): Catalog => ({
    sde_build: null,
    races: [
      { id: GALLENTE, name: 'Gallente' },
      { id: PIRATE, name: 'Pirate' },
    ],
    factions: [
      { id: FEDERATION, name: 'Gallente Federation', logo_url: 'fed.png', empire_of: GALLENTE },
      { id: SISTERS, name: 'Servant Sisters of EVE', logo_url: 'soe.png', empire_of: null },
      { id: GURISTAS, name: 'Guristas Pirates', logo_url: null, empire_of: null },
    ],
    groups: [],
    meta_groups: [],
    ships,
  });
  const named = (ships: ShipSummary[]) => {
    const { of } = shipBuilders(catalog(ships));
    return ships.map((one) => of(one).name);
  };

  it("names a race's empire ships by the race and the rest by their faction", () => {
    expect(
      named([ship(1, GALLENTE, FEDERATION), ship(2, GALLENTE, SISTERS), ship(3, GALLENTE, null)]),
    ).toEqual(['Gallente', 'Servant Sisters of EVE', 'Gallente']);
  });

  it('names pirate hulls by their faction, and by the race without one', () => {
    expect(named([ship(1, PIRATE, GURISTAS), ship(2, PIRATE, null)])).toEqual([
      'Guristas Pirates',
      'Pirate',
    ]);
  });

  it('falls back to the faction, then Unaffiliated, without a race', () => {
    expect(named([ship(1, null, GURISTAS), ship(2, null, null)])).toEqual([
      'Guristas Pirates',
      'Unaffiliated',
    ]);
  });

  it('lists the races with ships, then the factions by name, with emblems', () => {
    const ships = [
      ship(1, GALLENTE, FEDERATION),
      ship(2, GALLENTE, SISTERS),
      ship(3, PIRATE, GURISTAS),
      ship(4, null, null),
    ];
    expect(shipBuilders(catalog(ships)).all).toEqual([
      { key: `race:${GALLENTE}`, name: 'Gallente', logo: 'fed.png' },
      { key: `faction:${GURISTAS}`, name: 'Guristas Pirates', logo: null },
      { key: `faction:${SISTERS}`, name: 'Servant Sisters of EVE', logo: 'soe.png' },
    ]);
  });
});

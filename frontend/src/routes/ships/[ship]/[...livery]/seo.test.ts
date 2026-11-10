import { describe, expect, it } from 'vitest';
import type { ShipDetail, Skin, SkinrDesign } from '#lib/types.js';
import { shipSeo } from './seo';

const rifter = {
  id: 587,
  name: 'Rifter',
  description: 'The Rifter is a very powerful combat frigate. It has more to say.',
} as unknown as ShipDetail;
const skin = { id: 170, name: 'Krusual' } as unknown as Skin;
const design = {
  id: '9764301f61574a25e4fcb19b993532d85c8c6ef985ea761829b967c29dd2cbf8',
  name: 'Rock Raider',
  creator: { id: 1, name: 'Talanis Truelight' },
  min_price_plex: 1250,
} as unknown as SkinrDesign;

describe('shipSeo', () => {
  it('describes the ship with its first sentence and its own card', () => {
    const seo = shipSeo(rifter, null, null);
    expect(seo.title).toBe('Rifter skins');
    expect(seo.description).toContain('The Rifter is a very powerful combat frigate.');
    expect(seo.description).not.toContain('more to say');
    expect(seo.image).toBe('/og/ships/587.png');
    expect(seo.crumbs).toEqual([
      ['Ships', '/'],
      ['Rifter', '/ships/rifter'],
    ]);
  });

  it('names a skin and points at its card and page', () => {
    const seo = shipSeo(rifter, skin, null);
    expect(seo.title).toBe('Krusual Rifter skin');
    expect(seo.image).toBe('/og/ships/587/skins/170.png');
    expect(seo.crumbs.at(-1)).toEqual(['Krusual', '/ships/rifter/skins/krusual']);
  });

  it('names a design with its maker and price; a design wins over a skin', () => {
    const seo = shipSeo(rifter, skin, design);
    expect(seo.title).toBe('Rock Raider, a SKINR for the Rifter');
    expect(seo.description).toContain('by Talanis Truelight');
    expect(seo.description).toContain('From 1,250 PLEX');
    expect(seo.image).toBe(`/og/skinr/${design.id}.png`);
    expect(seo.crumbs.at(-1)).toEqual([
      'Rock Raider',
      '/ships/rifter/skinr/rock-raider-9764301f6157',
    ]);
  });

  it('says nothing of a price for a design not on sale', () => {
    const seo = shipSeo(rifter, null, { ...design, min_price_plex: null });
    expect(seo.description).not.toContain('PLEX');
  });
});

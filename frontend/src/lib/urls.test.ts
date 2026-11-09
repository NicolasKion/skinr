import { describe, expect, it } from 'vitest';
import {
  designHref,
  designerHref,
  designerId,
  parseLivery,
  shipHref,
  skinHref,
  slugify,
} from './urls';

describe('slugify', () => {
  it('turns a name into a URL segment', () => {
    expect(slugify('Kronos')).toBe('kronos');
    expect(slugify("Goru's Shuttle")).toBe('gorus-shuttle');
    expect(slugify("Capsule - Genolution 'Auroral' 197-variant")).toBe(
      'capsule-genolution-auroral-197-variant',
    );
    expect(slugify('Crème Brûlée')).toBe('creme-brulee');
  });
});

describe('ship and designer links', () => {
  it('names a ship', () => {
    expect(shipHref({ name: 'Megathron Navy Issue' }, '?skin=3')).toBe(
      '/ships/megathron-navy-issue?skin=3',
    );
  });

  it('names a designer and keeps their id', () => {
    expect(designerHref({ id: 95569887, name: 'Nicolas Kion' })).toBe(
      '/skinr/designer/nicolas-kion-95569887',
    );
    expect(designerHref({ id: 7, name: null })).toBe('/skinr/designer/7');
  });

  it('reads the id back, whatever the name says', () => {
    expect(designerId('nicolas-kion-95569887')).toBe(95569887);
    expect(designerId('95569887')).toBe(95569887);
    expect(designerId('old-name-12')).toBe(12);
    expect(designerId('no-id-here')).toBeNull();
  });
});

describe('liveries', () => {
  const kronos = { name: 'Kronos' };

  it('names a skin and a design on their ship', () => {
    expect(skinHref(kronos, { name: 'Headhunter' })).toBe('/ships/kronos/skins/headhunter');
    expect(
      designHref(kronos, { id: '109feb1d13bf80dd23058c118f58399f', name: 'Deep Void Matte' }),
    ).toBe('/ships/kronos/skinr/deep-void-matte-109feb1d13bf');
    expect(designHref(kronos, { id: '109feb1d13bf80dd', name: '67' })).toBe(
      '/ships/kronos/skinr/67-109feb1d13bf',
    );
  });

  it('reads them back', () => {
    expect(parseLivery('')).toEqual({ kind: 'none' });
    expect(parseLivery('skins/headhunter')).toEqual({ kind: 'skin', slug: 'headhunter' });
    expect(parseLivery('skinr/deep-void-matte-109feb1d13bf')).toEqual({
      kind: 'skinr',
      prefix: '109feb1d13bf',
    });
    expect(parseLivery('skinr/109feb1d13bf')).toEqual({ kind: 'skinr', prefix: '109feb1d13bf' });
  });

  it('refuses a path that names nothing', () => {
    for (const rest of ['skins', 'skinr/no-id-here', 'skinr/abc', 'other/x', 'skins/a/b']) {
      expect(parseLivery(rest)).toBeNull();
    }
  });
});

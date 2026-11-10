import { describe, expect, it } from 'vitest';
import { planPage, type PageStop } from './page-transitions';

const SHIP_ROUTE = '/ships/[ship]/[...livery]';

const stops: Record<string, PageStop> = {
  explorer: { routeId: '/', pathname: '/', href: 'https://skinr.space/' },
  rifter: {
    routeId: SHIP_ROUTE,
    pathname: '/ships/rifter',
    href: 'https://skinr.space/ships/rifter',
    ship: 'rifter',
  },
  rifterSkin: {
    routeId: SHIP_ROUTE,
    pathname: '/ships/rifter/skins/firewall',
    href: 'https://skinr.space/ships/rifter/skins/firewall',
    ship: 'rifter',
  },
  ibis: {
    routeId: SHIP_ROUTE,
    pathname: '/ships/ibis',
    href: 'https://skinr.space/ships/ibis',
    ship: 'ibis',
  },
  hub: { routeId: '/skinr', pathname: '/skinr', href: 'https://skinr.space/skinr' },
  designer: {
    routeId: '/skinr/designer/[designer]',
    pathname: '/skinr/designer/someone-1',
    href: 'https://skinr.space/skinr/designer/someone-1',
  },
  me: { routeId: '/me', pathname: '/me', href: 'https://skinr.space/me' },
  dev: {
    routeId: '/dev/skin-icons',
    pathname: '/dev/skin-icons',
    href: 'https://skinr.space/dev/skin-icons',
  },
};

const plan = (from: string, to: string, wide = true) => planPage(stops[from], stops[to], wide);

describe('planPage', () => {
  it('leaves the page alone when it stays (filters, a livery of the same ship)', () => {
    expect(plan('explorer', 'explorer')).toBeNull();
    expect(
      planPage(stops.hub, { ...stops.hub, href: `${stops.hub.href}?skinr=1` }, true),
    ).toBeNull();
    expect(plan('rifter', 'rifterSkin')).toBeNull();
  });

  it('sweeps between the explorer and a ship beside the panel', () => {
    expect(plan('explorer', 'rifter')).toEqual({ slide: 'sweep-forward', peekStatic: false });
    expect(plan('rifter', 'explorer')).toEqual({ slide: 'sweep-back', peekStatic: false });
  });

  it('slides them like any two areas on a phone', () => {
    expect(plan('explorer', 'rifter', false)?.slide).toBe('area-forward');
    expect(plan('rifter', 'explorer', false)?.slide).toBe('area-back');
  });

  it('slides the way the site bar reads between areas', () => {
    expect(plan('rifter', 'me')?.slide).toBe('area-forward');
    expect(plan('rifter', 'hub')?.slide).toBe('area-forward');
    expect(plan('me', 'rifter')?.slide).toBe('area-back');
    expect(plan('me', 'explorer')?.slide).toBe('area-back');
  });

  it('morphs a designer between the hub and their page', () => {
    expect(plan('hub', 'designer')?.slide).toBe('morph');
    expect(plan('designer', 'hub')?.slide).toBe('morph');
  });

  it('keeps the panel put between two pages that have one', () => {
    expect(plan('explorer', 'hub')).toEqual({ slide: 'area-forward', peekStatic: true });
    expect(plan('hub', 'me')?.peekStatic).toBe(true);
    expect(plan('rifter', 'me')?.peekStatic).toBe(false);
  });

  it('cross-fades within an area and to pages outside the site bar', () => {
    expect(plan('rifter', 'ibis')?.slide).toBe('fade');
    expect(plan('explorer', 'dev')?.slide).toBe('fade');
  });
});

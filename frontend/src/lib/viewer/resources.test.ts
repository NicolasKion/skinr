import { describe, expect, it, vi } from 'vitest';
import {
  createGate,
  gameFiles,
  isLight,
  resPath,
  resUrl,
  shipFileUrls,
  sofFiles,
  splitPath,
} from './resources';

describe('resUrl', () => {
  it('names a file under its build', () => {
    expect(resUrl('3667080', 'res:/dx9/model/ship/a.gr2')).toBe(
      '/cdn/res/3667080/dx9/model/ship/a.gr2',
    );
    expect(resUrl('1', 'dx9\\scene\\b c.red')).toBe('/cdn/res/1/dx9/scene/b%20c.red');
  });

  it('is one URL whatever the case of the path', () => {
    expect(resUrl('1', 'RES:/DX9/Model/Ship/A.gr2')).toBe('/cdn/res/1/dx9/model/ship/a.gr2');
  });
});

describe('resPath', () => {
  it('reads a path back out of its URL', () => {
    expect(resPath(resUrl('1', 'res:/dx9/scene/b c.red'))).toBe('res:/dx9/scene/b c.red');
  });
});

describe('isLight', () => {
  it('takes the catalog, graphs and shaders, not geometry or textures', () => {
    expect(isLight('/cdn/res/7/dx9/model/spaceobjectfactory/hulls/a.black')).toBe(true);
    expect(isLight('/cdn/res/7/dx9/model/ship/a_fx.red')).toBe(true);
    expect(isLight('/cdn/res/7/graphics/effect.dx11/managed/hull.sm_depth')).toBe(true);
    expect(isLight('/cdn/res/7/dx9/model/ship/a.gr2')).toBe(false);
    expect(isLight('/cdn/res/7/dx9/model/ship/a_d.dds')).toBe(false);
  });
});

describe('createGate', () => {
  it('lets a few in at a time, then everyone once opened', async () => {
    const gate = createGate(1);
    const order: string[] = [];
    const first = await gate.enter();
    const second = gate.enter().then((leave) => {
      order.push('second');
      return leave;
    });
    const third = gate.enter().then(() => order.push('third'));
    await Promise.resolve();
    expect(order).toEqual([]);

    first();
    await second;
    expect(order).toEqual(['second']);

    gate.open();
    await third;
    await gate.enter();
    expect(order).toEqual(['second', 'third']);
  });
});

describe('sofFiles', () => {
  it('names the catalog files of a DNA', () => {
    expect(sofFiles('MF4_T1:minmatarbase:minmatar?material?a;b')).toEqual([
      'res:/dx9/model/spaceobjectfactory/generic.black',
      'res:/dx9/model/spaceobjectfactory/hulls/mf4_t1.black',
      'res:/dx9/model/spaceobjectfactory/factions/minmatarbase.black',
      'res:/dx9/model/spaceobjectfactory/races/minmatar.black',
    ]);
  });
});

describe('shipFileUrls', () => {
  it('lists the catalog and the files worth fetching, once each', () => {
    const listed = [
      'res:/dx9/model/ship/a.gr2',
      'res:/fisfx/impact/insidemap_03a.dds',
      'res:/dx9/model/spaceobjectfactory/generic.black',
    ];

    expect(shipFileUrls('7', 'a:b:c', listed)).toEqual([
      '/cdn/res/7/dx9/model/spaceobjectfactory/generic.black',
      '/cdn/res/7/dx9/model/spaceobjectfactory/hulls/a.black',
      '/cdn/res/7/dx9/model/spaceobjectfactory/factions/b.black',
      '/cdn/res/7/dx9/model/spaceobjectfactory/races/c.black',
      '/cdn/res/7/dx9/model/ship/a.gr2',
    ]);
    expect(shipFileUrls('7', null, [])).toEqual([]);
  });
});

describe('splitPath', () => {
  it('splits a path into its folder and name', () => {
    expect(splitPath('res:/DX9/SOF/Materials/Steel.black')).toEqual({
      folder: 'dx9/sof/materials',
      name: 'steel.black',
    });
  });
});

describe('gameFiles', () => {
  const listingFetch = () =>
    vi.fn(async (url: string | URL | Request) => {
      if (String(url) === '/cdn/list/7/sof/materials') {
        return new Response(JSON.stringify(['glass.black', 'steel.black']));
      }
      return new Response('missing', { status: 404 });
    });

  it('answers exists from one listing per folder', async () => {
    const fetcher = listingFetch();
    const files = gameFiles('7', fetcher as unknown as typeof fetch);

    expect(await files.exists('res:/sof/materials/Steel.black')).toBe(true);
    expect(await files.exists('res:/sof/materials/rust.black')).toBe(false);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it('throws for a file the build does not have', async () => {
    const files = gameFiles('7', listingFetch() as unknown as typeof fetch);

    await expect(files.read('res:/dx9/nothing.dds')).rejects.toThrow('HTTP 404');
  });
});

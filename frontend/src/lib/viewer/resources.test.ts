import { describe, expect, it, vi } from 'vitest';
import { gameFiles, resUrl, splitPath } from './resources';

describe('resUrl', () => {
  it('names a file under its build', () => {
    expect(resUrl('3667080', 'res:/dx9/model/ship/a.gr2')).toBe(
      '/cdn/res/3667080/dx9/model/ship/a.gr2',
    );
    expect(resUrl('1', 'dx9\\scene\\b c.red')).toBe('/cdn/res/1/dx9/scene/b%20c.red');
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

import { describe, expect, it } from 'vitest';
import { standIn, withoutHitEffects, worthPreloading } from './unused';

describe('standIn', () => {
  it('reads the blurred cube for the backdrop, whatever the case', () => {
    expect(standIn('res:/dx9/Scene/Universe/c01_cube.dds')).toBe(
      'res:/dx9/scene/universe/c01_cube_blur.dds',
    );
  });

  it('leaves every other file alone', () => {
    expect(standIn('res:/dx9/scene/universe/c01_cube_refl.dds')).toBe(
      'res:/dx9/scene/universe/c01_cube_refl.dds',
    );
  });
});

describe('withoutHitEffects', () => {
  it('drops the impact overlay and keeps the rest', () => {
    const values = { mesh: { name: 'hull' }, impactOverlay: { name: 'hits' } };

    expect(withoutHitEffects(values)).toEqual({ mesh: { name: 'hull' }, impactOverlay: null });
    expect(values.impactOverlay).toEqual({ name: 'hits' });
  });
});

describe('worthPreloading', () => {
  it('skips hit effects and swaps the backdrop once', () => {
    const paths = [
      'res:/dx9/model/ship/minmatar/frigate/mf4/mf4_t1.gr2',
      'res:/fisfx/impact/insidemap_03a.dds',
      'res:/texture/fx/3d/3dfx_shield_03.dds',
      'res:/texture/fx/gradients/floatingpoint/shieldimpactgradient_01a.dds',
      'res:/graphics/generic/unitsphere/unitsphere_shieldgeo_01a.gr2',
      'res:/graphics/effect.dx11/managed/space/spaceobject/v5/fx/impact/fxshieldimpactv5.sm_depth',
      'res:/dx9/scene/universe/c01_cube.dds',
      'res:/dx9/scene/universe/c01_cube_blur.dds',
    ];

    expect(worthPreloading(paths)).toEqual([
      'res:/dx9/model/ship/minmatar/frigate/mf4/mf4_t1.gr2',
      'res:/dx9/scene/universe/c01_cube_blur.dds',
    ]);
  });
});

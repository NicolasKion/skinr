import { describe, expect, it } from 'vitest';
import {
  componentRuns,
  listingAudience,
  listingState,
  skinrIcon,
  skinrLook,
  skinrLookOn,
  skinrPatternName,
} from './skinr';
import type { SkinrComponent, SkinrDesign } from './types';

const material = (id: number, name: string): SkinrComponent => ({
  id,
  name,
  category: 'Material',
  finish: 'Matte',
  material: name,
  texture: null,
  projection_u: 'clamp-to-border',
  projection_v: 'clamp-to-border',
  icon_url: null,
  color: null,
});

const components: Record<string, SkinrComponent> = {
  1143: material(1143, 'cosm_grape_gloss_000_100_100'),
  209: material(209, 'cosm_blue_matt_000_100_001'),
  213: material(213, 'cosm_bw_matt_000_000_100'),
  188: {
    id: 188,
    name: 'Hex Recursion',
    category: 'Pattern',
    finish: 'Matte',
    material: null,
    texture: 'res:/texture/projection/cosm_hex_hex_2k.dds',
    projection_u: 'repeat',
    projection_v: 'clamp-to-border',
    icon_url: '/api/skinr/icons/188.png',
    color: null,
  },
};

/** The Blood Raider slot mapping from the SDE (faction 500012). */
const bloodRaider = { 1: 3, 2: 4, 3: 1, 4: 2 };

const transform = {
  position: { x: 1, y: 2, z: 3 },
  rotation: { x: 0, y: 0, z: 0, w: 1 },
  scaling: { x: 4, y: 4, z: 4 },
};

const design = (slots: SkinrDesign['layout']['slots']): SkinrDesign => ({
  id: 'FBAE29375B7ECBDC0C3547B7AA9C0F1180EE7F0E',
  name: 'Pacoman',
  line: null,
  tier: 5,
  ship_type_id: 95741,
  creator: { id: 1, name: null },
  layout: { slots },
  listings: 1,
  quantity: 1,
  min_price_plex: 69,
});

describe('skinrLook', () => {
  it('puts each material slot where the faction mapping says', () => {
    const look = skinrLook(
      'abc4_t1:bloodraider:amarr',
      design([
        { id: 1, configuration: { nanocoating: { id: 1143 } } },
        { id: 3, configuration: { nanocoating: { id: 213 } } },
      ]),
      components,
      bloodRaider,
    );
    // Slot 1 paints material3, slot 3 material1; slots 2 and 4 stay empty.
    expect(look.dna).toBe(
      'abc4_t1:bloodraider:amarr:material?cosm_bw_matt_000_000_100;none;cosm_grape_gloss_000_100_100;none',
    );
    expect(look.pattern).toBeNull();
  });

  it('turns patterns into a SOF pattern with the design transform', () => {
    const look = skinrLook(
      'abc4_t1:bloodraider:amarr',
      design([
        {
          id: 5,
          configuration: {
            pattern: {
              id: 188,
              configuration: {
                projection: { slot1: true, slot2: false, slot3: true, slot4: false },
                transform,
                mirrored: true,
              },
            },
          },
        },
        { id: 6, configuration: { nanocoating: { id: 209 } } },
      ]),
      components,
      bloodRaider,
    );
    const name = skinrPatternName('FBAE29375B7ECBDC0C3547B7AA9C0F1180EE7F0E');
    expect(name).toBe('skinr_fbae29375b7ecbdc0c3547b7');
    expect(look.dna).toBe(
      `abc4_t1:bloodraider:amarr:pattern?${name};cosm_blue_matt_000_100_001;none`,
    );
    const record = look.pattern?.record as Record<string, any>;
    expect(record.layer1).toMatchObject({
      textureName: 'PatternMask1Map',
      textureResFilePath: 'res:/texture/projection/cosm_hex_hex_2k.dds',
      projectionTypeU: 0,
      projectionTypeV: 2,
      materialSource: 4,
      // SKINR slots 1 and 3 are material3 and material1 on a Blood Raider hull.
      isTargetMtl3: true,
      isTargetMtl1: true,
      isTargetMtl4: false,
      isTargetMtl2: false,
    });
    expect(record.layer2).toBeUndefined();
    expect(record.projections).toEqual([
      {
        _type: 'EveSOFDataPatternPerHull',
        name: 'abc4_t1',
        transformLayer1: {
          _type: 'EveSOFDataPatternTransform',
          position: [1, 2, 3],
          rotation: [0, 0, 0, 1],
          scaling: [4, 4, 4],
          isMirrored: true,
        },
      },
    ]);
  });
});

describe('skinrIcon', () => {
  it('tints the hexagon with the design and lays its pattern over it', () => {
    const colored = {
      ...components,
      1143: { ...components[1143], color: '#e1e531' },
      209: { ...components[209], color: '#242221' },
    };
    const icon = skinrIcon(
      design([
        { id: 1, configuration: { nanocoating: { id: 1143 } } },
        { id: 3, configuration: { nanocoating: { id: 209 } } },
        { id: 5, configuration: { pattern: { id: 188, configuration: {} } } },
      ]),
      colored,
    );
    // No secondary material: the top band repeats the primary.
    expect(icon.colors).toEqual({ primary: '#e1e531', secondary: '#e1e531', hull: '#242221' });
    // The pattern's material slot (6) is empty, so it paints in the bare color.
    expect(icon.patterns).toEqual([{ url: '/api/skinr/icons/188.png', color: '#d8d8d8' }]);
  });
});

describe('skinrLookOn', () => {
  it("takes the slot mapping of the hull's faction", () => {
    const look = skinrLookOn(
      { dna: 'abc4_t1:amarrbase:amarr', faction_id: 500003 },
      design([{ id: 1, configuration: { nanocoating: { id: 1143 } } }]),
      components,
      // Amarr and Caldari, as the SDE maps them.
      { 500003: { 1: 4, 2: 1, 3: 2, 4: 3 }, 500001: { 1: 3, 2: 2, 3: 4, 4: 1 } },
    );
    expect(look.dna).toBe(
      'abc4_t1:amarrbase:amarr:material?none;none;none;cosm_grape_gloss_000_100_100',
    );
  });

  it('maps slots straight across for a hull without a faction', () => {
    const look = skinrLookOn(
      { dna: 'abc4_t1:amarrbase:amarr', faction_id: null },
      design([{ id: 1, configuration: { nanocoating: { id: 1143 } } }]),
      components,
      { 500003: { 1: 4, 2: 1, 3: 2, 4: 3 } },
    );
    expect(look.dna).toBe(
      'abc4_t1:amarrbase:amarr:material?cosm_grape_gloss_000_100_100;none;none;none',
    );
  });
});

describe('listingAudience', () => {
  it('reads the target in any of its likely shapes', () => {
    expect(listingAudience({ corporation_id: 98 })).toEqual({ kind: 'corporation', id: 98 });
    expect(listingAudience({ alliance: 99 })).toEqual({ kind: 'alliance', id: 99 });
    expect(listingAudience({ character: { id: 7 } })).toEqual({ kind: 'character', id: 7 });
  });

  it('treats a target without an id as public', () => {
    expect(listingAudience(null)).toEqual({ kind: 'public' });
    expect(listingAudience('public')).toEqual({ kind: 'public' });
    expect(listingAudience({ public: {} })).toEqual({ kind: 'public' });
  });
});

describe('listing words', () => {
  it('names states and runs the way the game does', () => {
    expect(listingState('sold_out')).toBe('Sold out');
    expect(listingState('mystery')).toBe('mystery');
    expect(componentRuns(1)).toBe('1 use left');
    expect(componentRuns(12)).toBe('12 uses left');
    expect(componentRuns(null)).toBe('Unlimited');
  });
});

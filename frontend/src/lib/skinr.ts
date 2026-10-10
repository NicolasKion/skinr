// A SKINR design, as the Space Object Factory can build it.
//
// SKINR (the game's player-made ship designs; esi-docs guide "Reading SKINR
// Data") puts components in eight slots: four materials (1-4), two patterns
// (5, 7) and the material each pattern paints with (6, 8). The SOF already
// knows both halves: SKINR materials are SOF materials, so the four material
// slots become a `material?…` DNA command, and a SOF pattern is two projected
// texture layers with a per-hull transform, which is exactly what a SKINR
// pattern is. So a design becomes a DNA string plus one synthetic pattern
// record, registered with the SOF under its own name before the build.

import type { Colors, SkinrComponent, SkinrDesign, SkinrSlotConfiguration } from './types';

/** The SKINR material slots, in slot order. */
const MATERIAL_SLOTS = [1, 2, 3, 4] as const;

/** Each SKINR pattern slot, the slot of the material it paints with, and its SOF layer. */
const PATTERN_SLOTS = [
  { pattern: 5, material: 6, layer: 1 },
  { pattern: 7, material: 8, layer: 2 },
] as const;

/** SKINR's projection modes as the SOF's ProjectionType enum. */
const PROJECTION_TYPE: Record<string, number> = {
  repeat: 0,
  'clamp-to-edge': 1,
  'clamp-to-border': 2,
};

/**
 * Where a SOF pattern layer takes its material from: after the four hull
 * materials come the pattern command's two (CCP's own patterns use 4 and 5).
 */
const LAYER_MATERIAL_SOURCE = { 1: 4, 2: 5 } as const;

/** Prefix of the synthetic pattern names, so they never collide with CCP's. */
const PATTERN_PREFIX = 'skinr_';
/** How much of the design id goes into the pattern name. */
const PATTERN_ID_LENGTH = 24;

/** A pattern laid over a SKIN hexagon: its icon (a white-on-black mask) and its paint. */
export interface IconPattern {
  url: string;
  color: string;
}

/** The paint of a pattern whose material slot is empty. */
const BARE_PATTERN_COLOR = '#d8d8d8';

/**
 * A design as the game's SKIN hexagon: its primary, secondary and detailing
 * materials tint the hexagon's bottom, top and middle bands, and each pattern
 * is laid over it in its own material's color.
 */
export function skinrIcon(
  design: SkinrDesign,
  components: Record<string, SkinrComponent>,
): { colors: Colors; patterns: IconPattern[] } {
  const slots = new Map(design.layout.slots.map((slot) => [slot.id, slot.configuration]));
  const colorOf = (slot: number) => {
    const id = slots.get(slot)?.nanocoating?.id;
    return (id !== undefined && components[id]?.color) || null;
  };
  const patterns: IconPattern[] = [];
  for (const { pattern: patternSlot, material } of PATTERN_SLOTS) {
    const id = slots.get(patternSlot)?.pattern?.id;
    const url = id !== undefined ? components[id]?.icon_url : null;
    if (url) patterns.push({ url, color: colorOf(material) ?? BARE_PATTERN_COLOR });
  }
  return {
    colors: {
      primary: colorOf(1),
      secondary: colorOf(2) ?? colorOf(1),
      hull: colorOf(3) ?? colorOf(4) ?? colorOf(1),
    },
    patterns,
  };
}

/** What the SOF needs to build a design. */
export interface SkinrLook {
  dna: string;
  /** The pattern to register before building, when the design has one. */
  pattern: { name: string; record: object } | null;
}

/** The SOF pattern name a design is registered under. */
export function skinrPatternName(designId: string): string {
  return PATTERN_PREFIX + designId.slice(0, PATTERN_ID_LENGTH).toLowerCase();
}

/**
 * Builds a design on top of a ship's own DNA (`hull:faction:race`).
 * `slotMaterials` maps SKINR material slots to Trinity's material1-4 for the
 * hull's faction; without one the slots map straight across.
 */
export function skinrLook(
  baseDna: string,
  design: SkinrDesign,
  components: Record<string, SkinrComponent>,
  slotMaterials: Record<string, number>,
): SkinrLook {
  const [hulls, faction, race] = baseDna.split(':');
  const slots = new Map<number, SkinrSlotConfiguration>(
    design.layout.slots.map((slot) => [slot.id, slot.configuration]),
  );
  const materialOf = (slot: number) => {
    const id = slots.get(slot)?.nanocoating?.id;
    return (id !== undefined && components[id]?.material) || 'none';
  };
  const trinityMaterial = (slot: number) => slotMaterials[slot] ?? slot;

  const materials = ['none', 'none', 'none', 'none'];
  for (const slot of MATERIAL_SLOTS) materials[trinityMaterial(slot) - 1] = materialOf(slot);

  const layers: Record<string, object> = {};
  const transforms: Record<string, object> = {};
  for (const { pattern: patternSlot, layer } of PATTERN_SLOTS) {
    const pattern = slots.get(patternSlot)?.pattern;
    const component = pattern ? components[pattern.id] : undefined;
    if (!pattern || !component?.texture) continue;
    const { projection = {}, transform, mirrored } = pattern.configuration;
    const targets: Record<string, boolean> = {};
    for (const slot of MATERIAL_SLOTS) {
      targets[`isTargetMtl${trinityMaterial(slot)}`] = projection[`slot${slot}`] === true;
    }
    layers[`layer${layer}`] = {
      _type: 'EveSOFDataPatternLayer',
      textureName: `PatternMask${layer}Map`,
      textureResFilePath: component.texture,
      projectionTypeU: PROJECTION_TYPE[component.projection_u ?? ''] ?? PROJECTION_TYPE.repeat,
      projectionTypeV: PROJECTION_TYPE[component.projection_v ?? ''] ?? PROJECTION_TYPE.repeat,
      materialSource: LAYER_MATERIAL_SOURCE[layer],
      ...targets,
    };
    if (transform) {
      transforms[`transformLayer${layer}`] = {
        _type: 'EveSOFDataPatternTransform',
        position: [transform.position.x, transform.position.y, transform.position.z],
        rotation: [
          transform.rotation.x,
          transform.rotation.y,
          transform.rotation.z,
          transform.rotation.w,
        ],
        scaling: [transform.scaling.x, transform.scaling.y, transform.scaling.z],
        isMirrored: mirrored === true,
      };
    }
  }

  let dna = `${hulls}:${faction}:${race}`;
  if (materials.some((material) => material !== 'none')) dna += `:material?${materials.join(';')}`;
  if (Object.keys(layers).length === 0) return { dna, pattern: null };

  const name = skinrPatternName(design.id);
  const record = {
    _type: 'EveSOFDataPattern',
    name,
    ...layers,
    // A design is made for one hull; every hull in the DNA gets its transforms.
    projections: hulls
      .split(';')
      .map((hull) => Object.assign({ _type: 'EveSOFDataPatternPerHull', name: hull }, transforms)),
  };
  dna += `:pattern?${name};${materialOf(PATTERN_SLOTS[0].material)};${materialOf(PATTERN_SLOTS[1].material)}`;
  return { dna, pattern: { name, record } };
}

// The JSON shapes of the Axum API (src/server/api.rs).

export interface Named {
  id: number;
  name: string;
}

export interface MetaGroup extends Named {
  color: string | null;
}

/** A livery's swatch colors as #rrggbb, from its SDE material set. */
export interface Colors {
  primary: string | null;
  secondary: string | null;
  hull: string | null;
}

/** A faction and its emblem, extracted from the game client by the import. */
export interface FactionInfo {
  id: number;
  name: string;
  logo_url: string | null;
  /** The race this faction is the home empire of (Amarr, Caldari, Gallente, Minmatar). */
  empire_of: number | null;
}

/** A ship's faction in full, for the blueprint's faction panel. */
export interface FactionDetail extends FactionInfo {
  /** A one-line motto; many factions have none. */
  tagline: string | null;
  description: string;
}

export interface ShipSummary {
  id: number;
  name: string;
  group_id: number;
  race_id: number | null;
  faction_id: number | null;
  meta_group_id: number | null;
  tech_level: number | null;
  dna: string;
  colors: Colors;
}

export interface Catalog {
  sde_build: string | null;
  races: Named[];
  factions: FactionInfo[];
  groups: Named[];
  meta_groups: MetaGroup[];
  ships: ShipSummary[];
}

export interface Skin {
  id: number;
  name: string;
  dna: string;
  colors: Colors;
  /** The game's icon for the skin material, when the client has one. */
  icon_url: string | null;
  /** That icon's colors, read off it by the import, to draw it sharp as vectors. */
  icon_colors: IconColors | null;
}

/** A skin icon's colors: its three bands and its lights. */
export interface IconColors extends Colors {
  lights: string;
}

/**
 * A run of EVE HTML text with its styles, or a line break (see the Rust
 * eve_html module). Rendered as text, never as markup.
 */
export type RichSegment =
  | {
      text: string;
      bold?: boolean;
      italic?: boolean;
      underline?: boolean;
      /** A validated CSS color, #rrggbbaa. */
      color?: string;
      /** A ship this text links to. */
      type_id?: number;
    }
  | { br: true };

export interface ShipDetail {
  id: number;
  name: string;
  /** The description as plain text. */
  description: string;
  description_rich: RichSegment[];
  group: Named;
  race: Named | null;
  faction: FactionDetail | null;
  meta_group: MetaGroup | null;
  tech_level: number | null;
  mass: number | null;
  volume: number | null;
  capacity: number | null;
  radius: number | null;
  dna: string;
  colors: Colors;
  skins: Skin[];
  sheet: Sheet;
  bonuses: BonusGroup[];
}

/** A ship's spec sheet (src/sheet.rs): figures as the game's Show Info prints them. */
export interface Sheet {
  slots: { high: number; mid: number; low: number; rig: number; turret: number; launcher: number };
  fitting: {
    powergrid: number | null;
    cpu: number | null;
    calibration: number | null;
    drone_bay: number | null;
    drone_bandwidth: number | null;
    fighter_bay: number | null;
    fighter_tubes: number | null;
  };
  /** Shield, armor, structure; resistances in percent. */
  defense: {
    name: string;
    hp: number;
    em: number;
    thermal: number;
    kinetic: number;
    explosive: number;
    recharge?: number;
  }[];
  capacitor: { capacity: number | null; recharge: number | null };
  targeting: {
    range: number | null;
    max_targets: number | null;
    scan_resolution: number | null;
    signature_radius: number | null;
    sensor: { kind: string; strength: number } | null;
  };
  navigation: {
    max_velocity: number | null;
    warp_speed: number | null;
    align_time: number | null;
    inertia: number | null;
  };
}

export interface BonusGroup {
  title: string;
  per_level: boolean;
  lines: { amount: string | null; text: RichSegment[] }[];
}

/** A SKINR component (src/server/skinr.rs): a material or a projected pattern. */
export interface SkinrComponent {
  id: number;
  name: string;
  /** `Material`, `Pattern` or `Metallic`. */
  category: string;
  finish: string | null;
  /** For materials: the SOF material name DNA uses. */
  material: string | null;
  /** For patterns: the projection texture. */
  texture: string | null;
  projection_u: string | null;
  projection_v: string | null;
  icon_url: string | null;
  /** For materials: a swatch color sampled from the client's icon, `#rrggbb`. */
  color: string | null;
}

export interface SkinrVector {
  x: number;
  y: number;
  z: number;
}

/** What one SKINR slot holds, exactly as ESI returns it. */
export interface SkinrSlotConfiguration {
  nanocoating?: { id: number };
  pattern?: {
    id: number;
    configuration: {
      /** Which of the four SKINR material slots the pattern is projected onto. */
      projection?: Partial<Record<'slot1' | 'slot2' | 'slot3' | 'slot4', boolean>>;
      transform?: {
        position: SkinrVector;
        rotation: SkinrVector & { w: number };
        scaling: SkinrVector;
      };
      mirrored?: boolean;
    };
  };
}

/** A player-made design (most of them listed on the Paragon Hub). */
export interface SkinrDesign {
  id: string;
  name: string;
  line: string | null;
  tier: number | null;
  /** The hull it is made for. */
  ship_type_id: number;
  creator: Person;
  layout: {
    slots: { id: number; configuration: SkinrSlotConfiguration }[];
    pattern_blend_mode?: string;
  };
  listings: number;
  quantity: number;
  min_price_plex: number | null;
}

/** `/api/ships/{id}/skinrs`. */
export interface ShipSkinrs {
  /** SKINR material slot (1-4) to Trinity hull material (1-4), for this hull's faction. */
  slot_materials: Record<string, number>;
  designs: SkinrDesign[];
  components: Record<string, SkinrComponent>;
}

/** A character, by id; the name is null until the API has looked it up. */
export interface Person {
  id: number;
  name: string | null;
}

/** For each faction id: SKINR material slot to Trinity hull material. */
export type FactionSlotMaterials = Record<string, Record<string, number>>;

/** `/api/me`: who is logged in, and where a login starts (null: this server has none). */
export interface Me {
  character: { id: number; name: string } | null;
  /** `/eve/login` on the SSO callback's origin, so the login cookie lands where the SSO returns. */
  login: string | null;
}

/** A Paragon Hub listing of a design. */
export interface SkinrListing {
  id: string;
  skinr_id: string;
  /** `listed`, `sold_out`, `expired` or `removed`. */
  state: string;
  price_plex: number | null;
  quantity: number;
  created: string;
  expires: string;
  /** Who it is offered to, exactly as ESI says it. */
  target: unknown;
}

/** A listing offered to the character privately, or to their corporation or alliance. */
export interface SkinrOffer extends SkinrListing {
  audience: 'character' | 'corporation' | 'alliance';
  seller: Person;
}

/** `/api/me/skinr`: a logged-in character's SKINR, from ESI. */
export interface MySkinr {
  character: {
    id: number;
    name: string;
    corporation_id: number | null;
    alliance_id: number | null;
  };
  /** Every design the lists refer to. */
  designs: SkinrDesign[];
  components: Record<string, SkinrComponent>;
  slot_materials: FactionSlotMaterials;
  /** Ids of the designs the character made. */
  created: string[];
  licenses: { skinr_id: string; activated: boolean; unactivated: number }[];
  /** One per component; `runs` is the uses left across its licenses, null for unlimited. */
  component_licenses: { component_id: number; type: string; runs: number | null }[];
  listings: SkinrListing[];
  offers: SkinrOffer[];
  /** Parts ESI could not answer this time. */
  unavailable: string[];
}

/** A designer with designs on sale. */
export interface Designer extends Person {
  designs: number;
  listings: number;
}

/** `/api/skinr`: a page of the Paragon Hub. */
export interface HubPage {
  designs: SkinrDesign[];
  components: Record<string, SkinrComponent>;
  slot_materials: FactionSlotMaterials;
  total: number;
  page: number;
  page_size: number;
  /** The designer filtered by. */
  designer: Designer | null;
  /** The busiest designers, when not filtering by one. */
  designers: Designer[];
}

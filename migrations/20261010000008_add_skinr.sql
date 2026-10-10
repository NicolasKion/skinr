-- SKINR, the game's player-made ship designs.

-- From the SDE: what each component is, and which Trinity hull material a
-- SKINR material slot paints on a hull of each faction.
create table skinr_components (
    id integer primary key,
    name text not null,
    category text not null,
    finish text,
    rarity integer,
    resource_file text not null,
    icon_file text,
    projection_u text,
    projection_v text
);

create table skinr_slot_materials (
    faction_id integer not null,
    slot_id integer not null,
    material_id integer not null,
    primary key (faction_id, slot_id)
);

-- The client's icon for each component (iconFile), fetched by the import.
create table skinr_component_icons (
    component_id integer primary key,
    png bytea not null
);

-- From ESI: designs listed on the Paragon Hub, their listings, and the names
-- of the characters who made and sell them. Kept current by the API's sync.
create table skinr_designs (
    id text primary key,
    name text not null,
    line text,
    creator_id bigint not null,
    ship_type_id integer not null,
    tier integer,
    -- The design's slots exactly as ESI returns them (`layout`).
    layout text not null,
    fetched_at timestamptz not null default now()
);

create index skinr_designs_ship_type_id on skinr_designs (ship_type_id);

create table skinr_listings (
    id text primary key,
    skinr_id text not null,
    seller_id bigint not null,
    state text not null,
    -- The asking price in PLEX, when the listing is priced in PLEX.
    price_plex bigint,
    quantity bigint not null,
    created timestamptz not null,
    expires timestamptz not null,
    last_modified timestamptz not null
);

create index skinr_listings_skinr_id on skinr_listings (skinr_id);

create table esi_characters (
    id bigint primary key,
    name text not null
);

-- Small pieces of sync state, such as the Paragon Hub cursor.
create table esi_sync_state (
    key text primary key,
    value text not null
);

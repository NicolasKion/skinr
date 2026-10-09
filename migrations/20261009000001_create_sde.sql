-- Reference data imported from CCP's Static Data Export by `sde_import`.
-- Only what the viewer uses: ships, how to build them (SOF DNA) and their skins.

create table sde_meta (
    key text primary key,
    value text not null
);

create table races (
    id integer primary key,
    name text not null,
    description text not null default ''
);

create table meta_groups (
    id integer primary key,
    name text not null,
    -- The in-game tint as #rrggbb, when the SDE gives one.
    color text
);

create table ship_groups (
    id integer primary key,
    name text not null
);

create table ships (
    id integer primary key,
    name text not null,
    description text not null default '',
    group_id integer not null references ship_groups (id),
    race_id integer references races (id),
    meta_group_id integer references meta_groups (id),
    tech_level integer,
    mass double precision,
    volume double precision,
    capacity double precision,
    radius double precision,
    -- The Space Object Factory parts; hull:faction:race is the default DNA.
    sof_hull text not null,
    sof_faction text not null,
    sof_race text not null
);

create index ships_group_id on ships (group_id);
create index ships_race_id on ships (race_id);

-- One row per ship and distinct SOF faction a skin swaps in.
create table ship_skins (
    ship_id integer not null references ships (id) on delete cascade,
    skin_id integer not null,
    name text not null,
    sof_faction text not null,
    primary key (ship_id, skin_id)
);

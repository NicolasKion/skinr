-- Factions and the emblem each ship flies under. `logo` is the SDE's
-- flatLogo name; faction_logos holds the PNG the import extracted from the
-- game client for it (see src/sde/logos.rs), kept across reseeds.
create table factions (
    id integer primary key,
    name text not null,
    logo text
);

alter table ships add column faction_id integer references factions (id);

create table faction_logos (
    name text primary key,
    png bytea not null,
    width integer not null,
    height integer not null
);

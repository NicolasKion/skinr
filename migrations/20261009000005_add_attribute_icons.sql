-- The game client's own icon for each spec sheet attribute, by attribute
-- name (see src/sheet.rs), fetched by the import like the faction logos.
create table attribute_icons (
    name text primary key,
    png bytea not null
);

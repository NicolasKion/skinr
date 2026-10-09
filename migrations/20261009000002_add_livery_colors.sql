-- The swatch colors of a livery (#rrggbb), from its SDE material set: the
-- default livery's on ships, each skin's on ship_skins. Null when the SDE
-- gives none.
alter table ships
    add column color_primary text,
    add column color_secondary text,
    add column color_hull text;

alter table ship_skins
    add column color_primary text,
    add column color_secondary text,
    add column color_hull text;

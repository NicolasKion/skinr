-- A swatch color for each SKINR material, sampled from the client's icon of
-- it (a shaded sphere in the material), as "#rrggbb".
alter table skinr_components add column color text;

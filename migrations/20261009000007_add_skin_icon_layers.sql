-- The greyscale layers the client tints into a skin icon
-- (res:/ui/texture/classes/skins/skin-icon-<layer>.png), for liveries
-- without a material icon of their own: a ship's standard issue.
create table skin_icon_layers (
    name text primary key,
    png bytea not null
);

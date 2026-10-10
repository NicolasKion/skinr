-- The colors of each in-game skin, read back off its icon by the import
-- (src/sde/skin_icon_colors.rs), as JSON: primary, secondary, hull, lights.
alter table skin_icons add column colors text;

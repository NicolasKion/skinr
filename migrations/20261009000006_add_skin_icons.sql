-- The skin material behind each livery, and the game client's icon for each
-- material (res:/ui/texture/classes/skins/icons/<material>.png).
alter table ship_skins add column material_id integer;

create table skin_icons (
    material_id integer primary key,
    png bytea not null
);

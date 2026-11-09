-- The SOF DNA commands a skin's material set adds after hull:faction:race
-- (materials, projected pattern, resource path insert), joined by ':'.
-- Empty until the next SDE import fills it.
alter table ship_skins add column sof_commands text not null default '';

-- The SOF DNA commands of the material set a hull's graphic wears by default
-- (tournament prize ships and special editions), joined by ':'. Empty until
-- the next SDE import fills it.
alter table ships add column sof_commands text not null default '';

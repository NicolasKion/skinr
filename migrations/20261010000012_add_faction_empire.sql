-- The race a faction is the home empire of (the SDE's empires: a faction
-- with a militia and a single member race), so ships it builds are named by
-- their race and every other faction's ships by the faction.
alter table factions add column empire_of integer;

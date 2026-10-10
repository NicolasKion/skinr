-- A faction's description and one-line tagline from the SDE, as EVE HTML,
-- for the blueprint's faction panel. Many factions have no tagline.
alter table factions add column description text not null default '';
alter table factions add column tagline text;

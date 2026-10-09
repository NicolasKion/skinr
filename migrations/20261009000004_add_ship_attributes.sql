-- The dogma attributes the spec sheet reads (by name, see src/sheet.rs) and
-- each ship's bonuses as the SDE's typeBonus lists them.
create table ship_attributes (
    ship_id integer not null references ships (id) on delete cascade,
    name text not null,
    value double precision not null,
    primary key (ship_id, name)
);

create table ship_bonuses (
    ship_id integer not null references ships (id) on delete cascade,
    position integer not null,
    -- 'skill' (per level of skill_id), 'role' or 'misc'.
    kind text not null,
    skill_id integer,
    skill_name text,
    bonus double precision,
    unit text,
    -- EVE HTML, rendered through eve_html.
    text text not null,
    primary key (ship_id, position)
);

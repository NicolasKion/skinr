-- The import now also fills the skins' and ships' SOF commands (and more of
-- the SKINR data), which an already seeded build lacks: forget that build so
-- the next import reseeds instead of skipping.
delete from sde_meta where key = 'seeded_build';

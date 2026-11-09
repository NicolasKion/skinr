//! ESI, CCP's REST API: a client that follows ESI's rules, the sync that
//! keeps the Paragon Hub's SKINR designs in Postgres, and a logged-in
//! character's own SKINR data.

pub mod cache;
pub mod character;
pub mod client;
pub mod skinr;

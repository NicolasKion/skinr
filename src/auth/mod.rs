//! Log in with EVE: the SSO client and server-side sessions.

pub mod session;
pub mod sso;

/// The one scope the site asks for. It reads a character's SKINR licenses,
/// component licenses and Paragon Hub listings, and the listings offered to
/// them, their corporation and their alliance. Checked against ESI's spec.
pub const COSMETIC_READ: &str = "esi.cosmetic.char:read";

/// Everything a login asks for; the app's registration must list the same.
pub const LOGIN_SCOPES: [&str; 1] = [COSMETIC_READ];

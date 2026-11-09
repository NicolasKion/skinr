//! What ESI tells a logged-in character about their SKINR: the licenses and
//! component licenses they own, the listings they posted on the Paragon Hub,
//! and the listings offered to them, their corporation or their alliance.
//! All of it needs an access token with `esi.cosmetic.char:read`; answers
//! are cached per character (`super::cache`).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::cache::{self, Error};
use super::client::EsiClient;

/// Paragon Hub listing routes do not say how long their answers keep; a
/// character's own lists are asked for again after this long.
const LISTINGS_KEEP: Duration = Duration::from_secs(5 * 60);
/// The shortest time a single answer is kept when ESI does not say.
const ANSWER_FLOOR: Duration = Duration::from_secs(60);

/// Where a character's cached answers live in `esi_cache`.
pub fn cache_prefix(character_id: i64) -> String {
    format!("character:{character_id}:")
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Licenses {
    #[serde(default)]
    pub licenses: Vec<License>,
}

/// A SKINR license the character owns.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct License {
    pub skinr_id: String,
    /// Whether one copy is applied to the character's ship.
    pub activated: bool,
    /// Copies still in the hangar.
    pub unactivated: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ComponentLicenses {
    #[serde(default)]
    pub licenses: Vec<ComponentLicense>,
}

/// A license for one SKINR component (a nanocoating or a pattern).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComponentLicense {
    pub component_id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    pub runs: Runs,
}

/// How often a component license can still be used, as ESI gives it:
/// `{"unlimited": true}` or `{"remaining": 6}`. The spec leaves the field
/// untyped, so a bare count is read too rather than losing the whole list.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(from = "RunsWire")]
pub struct Runs {
    pub unlimited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<u32>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RunsWire {
    Count(u32),
    Detail {
        #[serde(default)]
        unlimited: bool,
        #[serde(default)]
        remaining: Option<u32>,
    },
}

impl From<RunsWire> for Runs {
    fn from(wire: RunsWire) -> Self {
        match wire {
            RunsWire::Count(remaining) => Self {
                unlimited: false,
                remaining: Some(remaining),
            },
            RunsWire::Detail {
                unlimited,
                remaining,
            } => Self {
                unlimited,
                remaining,
            },
        }
    }
}

impl Runs {
    /// The uses left, `None` for unlimited (a license with no count is too).
    pub fn left(self) -> Option<u32> {
        if self.unlimited { None } else { self.remaining }
    }
}

/// A Paragon Hub listing as the character routes return it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Listing {
    pub id: String,
    pub skinr_id: String,
    pub seller_id: i64,
    pub state: String,
    #[serde(default)]
    pub price: serde_json::Value,
    pub quantity: i64,
    pub created: String,
    pub expires: String,
    /// Who the listing is visible to (only on the character's own listings).
    #[serde(default)]
    pub target: serde_json::Value,
}

impl Listing {
    /// The asking price in PLEX, when the listing is priced in PLEX.
    pub fn price_plex(&self) -> Option<i64> {
        self.price.get("plex")?.as_i64()
    }
}

/// The public part of a character: who they fly for.
#[derive(Debug, Deserialize, Serialize)]
pub struct Affiliation {
    pub corporation_id: i64,
    #[serde(default)]
    pub alliance_id: Option<i64>,
}

/// The character's ESI calls, with their access token.
pub struct CharacterEsi<'a> {
    pub pool: &'a PgPool,
    pub esi: &'a EsiClient,
    pub character_id: i64,
    pub token: &'a str,
}

impl CharacterEsi<'_> {
    fn key(&self, what: &str) -> String {
        format!("{}{what}", cache_prefix(self.character_id))
    }

    pub async fn licenses(&self) -> Result<Vec<License>, Error> {
        let path = format!("/characters/{}/cosmetics/skinr", self.character_id);
        let answer: Licenses = cache::get(
            self.pool,
            self.esi,
            &self.key("licenses"),
            &path,
            Some(self.token),
            ANSWER_FLOOR,
        )
        .await?;
        Ok(answer.licenses)
    }

    pub async fn component_licenses(&self) -> Result<Vec<ComponentLicense>, Error> {
        let path = format!(
            "/characters/{}/cosmetics/skinr/components",
            self.character_id
        );
        let answer: ComponentLicenses = cache::get(
            self.pool,
            self.esi,
            &self.key("components"),
            &path,
            Some(self.token),
            ANSWER_FLOOR,
        )
        .await?;
        Ok(answer.licenses)
    }

    /// The listings the character posted.
    pub async fn listings(&self) -> Result<Vec<Listing>, Error> {
        let path = format!("/characters/{}/paragon-hub/skinr", self.character_id);
        cache::listings(
            self.pool,
            self.esi,
            &self.key("listings"),
            &path,
            Some(self.token),
            LISTINGS_KEEP,
        )
        .await
    }

    /// Listings on the Paragon Hub visible only to the character.
    pub async fn offers(&self) -> Result<Vec<Listing>, Error> {
        let path = format!("/paragon-hub/skinr/characters/{}", self.character_id);
        cache::listings(
            self.pool,
            self.esi,
            &self.key("offers"),
            &path,
            Some(self.token),
            LISTINGS_KEEP,
        )
        .await
    }

    pub async fn corporation_offers(&self, corporation_id: i64) -> Result<Vec<Listing>, Error> {
        let path = format!("/paragon-hub/skinr/corporations/{corporation_id}");
        cache::listings(
            self.pool,
            self.esi,
            &self.key(&format!("corporation:{corporation_id}")),
            &path,
            Some(self.token),
            LISTINGS_KEEP,
        )
        .await
    }

    pub async fn alliance_offers(&self, alliance_id: i64) -> Result<Vec<Listing>, Error> {
        let path = format!("/paragon-hub/skinr/alliances/{alliance_id}");
        cache::listings(
            self.pool,
            self.esi,
            &self.key(&format!("alliance:{alliance_id}")),
            &path,
            Some(self.token),
            LISTINGS_KEEP,
        )
        .await
    }

    /// The character's corporation and alliance (a public route).
    pub async fn affiliation(&self) -> Result<Affiliation, Error> {
        let path = format!("/characters/{}", self.character_id);
        cache::get(
            self.pool,
            self.esi,
            &self.key("public"),
            &path,
            None,
            ANSWER_FLOOR,
        )
        .await
    }
}

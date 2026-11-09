//! `/api/me/skinr`: everything about SKINR for the logged-in character. ESI
//! gives the licenses they own, their component licenses, their Paragon Hub
//! listings and the listings offered to them, their corporation and their
//! alliance; the designs behind all of those are fetched once and kept with
//! the hub's. ESI has no list of the designs a character made, so "created"
//! holds the ones of theirs this site has seen (listed, owned or offered).

use std::collections::BTreeSet;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde::Serialize;

use super::AppState;
use super::api::{database_error, error};
use super::skinr::{
    ComponentOut, DesignOut, SlotMaterials, load_components, load_designs, load_slot_materials,
};
use crate::auth::session::{self, TokenError};
use crate::auth::{COSMETIC_READ, sso::SsoClient};
use crate::esi::character::{CharacterEsi, ComponentLicense, License, Listing};
use crate::esi::skinr::{fetch_missing_designs, name_characters};

/// Designs fetched from ESI for one request at most; the rest come on the next visit.
const MAX_NEW_DESIGNS: usize = 100;

/// What a character can sequence with one component: one entry however many
/// licenses they hold for it.
#[derive(Debug, Serialize, PartialEq, Eq)]
struct ComponentLicenseOut {
    component_id: i64,
    #[serde(rename = "type")]
    kind: String,
    /// The uses left across the licenses, `None` when any is unlimited.
    runs: Option<u32>,
}

/// One entry per component: unlimited if any license is, else the uses left
/// added up. Ordered by kind, then component.
fn merge_component_licenses(licenses: &[ComponentLicense]) -> Vec<ComponentLicenseOut> {
    let mut merged: std::collections::BTreeMap<(&str, i64), Option<u32>> =
        std::collections::BTreeMap::new();
    for license in licenses {
        let left = license.runs.left();
        merged
            .entry((license.kind.as_str(), license.component_id))
            .and_modify(|runs| *runs = runs.zip(left).map(|(a, b)| a.saturating_add(b)))
            .or_insert(left);
    }
    merged
        .into_iter()
        .map(|((kind, component_id), runs)| ComponentLicenseOut {
            component_id,
            kind: kind.to_owned(),
            runs,
        })
        .collect()
}

#[derive(Serialize)]
pub struct MySkinr {
    character: CharacterOut,
    /// Every design the lists below refer to, by the order of first mention.
    designs: Vec<DesignOut>,
    components: std::collections::BTreeMap<i32, ComponentOut>,
    slot_materials: SlotMaterials,
    /// Design ids this character created.
    created: Vec<String>,
    licenses: Vec<License>,
    component_licenses: Vec<ComponentLicenseOut>,
    listings: Vec<ListingOut>,
    offers: Vec<OfferOut>,
    /// Parts ESI could not answer this time; the rest is still shown.
    unavailable: Vec<&'static str>,
}

#[derive(Serialize)]
struct CharacterOut {
    id: i64,
    name: String,
    corporation_id: Option<i64>,
    alliance_id: Option<i64>,
}

#[derive(Serialize)]
struct ListingOut {
    id: String,
    skinr_id: String,
    state: String,
    price_plex: Option<i64>,
    quantity: i64,
    created: String,
    expires: String,
    /// Who it is offered to, as ESI says it (public, or a character, corporation or alliance).
    target: serde_json::Value,
}

impl From<Listing> for ListingOut {
    fn from(listing: Listing) -> Self {
        Self {
            price_plex: listing.price_plex(),
            id: listing.id,
            skinr_id: listing.skinr_id,
            state: listing.state,
            quantity: listing.quantity,
            created: listing.created,
            expires: listing.expires,
            target: listing.target,
        }
    }
}

/// A listing offered to the character privately, or to their corporation or alliance.
#[derive(Serialize)]
struct OfferOut {
    audience: &'static str,
    seller: super::skinr::Person,
    #[serde(flatten)]
    listing: ListingOut,
}

fn logged_out() -> Response {
    error(
        StatusCode::UNAUTHORIZED,
        "Log in with EVE to see your SKINRs.",
    )
}

/// Unwraps one ESI part; a failure is logged and noted as unavailable.
fn part<T: Default>(
    name: &'static str,
    result: Result<T, crate::esi::cache::Error>,
    unavailable: &mut Vec<&'static str>,
) -> T {
    result.unwrap_or_else(|cause| {
        tracing::warn!(error = %cause, part = name, "ESI could not answer");
        unavailable.push(name);
        T::default()
    })
}

pub async fn my_skinr(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<MySkinr>, Response> {
    let Some(sso) = state.sso.as_ref() else {
        return Err(logged_out());
    };
    let Some(character) = session::current(&state.pool, &headers)
        .await
        .map_err(database_error)?
    else {
        return Err(logged_out());
    };
    if !character.scopes.iter().any(|scope| scope == COSMETIC_READ) {
        return Err(error(
            StatusCode::FORBIDDEN,
            "Log in again and allow access to your cosmetics.",
        ));
    }
    let token = token_for(&state, sso, character.id).await?;
    let esi = CharacterEsi {
        pool: &state.pool,
        esi: &state.esi,
        character_id: character.id,
        token: &token,
    };

    let mut unavailable = Vec::new();
    let (licenses, component_licenses, listings, offers, affiliation) = tokio::join!(
        esi.licenses(),
        esi.component_licenses(),
        esi.listings(),
        esi.offers(),
        esi.affiliation(),
    );
    let licenses = part("licenses", licenses, &mut unavailable);
    let component_licenses = part("components", component_licenses, &mut unavailable);
    let listings = part("listings", listings, &mut unavailable);
    let offers = part("offers", offers, &mut unavailable);
    let affiliation = affiliation
        .map_err(|cause| {
            tracing::warn!(error = %cause, "could not read the character's affiliation");
            unavailable.push("affiliation");
        })
        .ok();

    let mut all_offers: Vec<(&'static str, Listing)> = offers
        .into_iter()
        .map(|offer| ("character", offer))
        .collect();
    if let Some(affiliation) = &affiliation {
        let corporation = esi.corporation_offers(affiliation.corporation_id).await;
        all_offers.extend(
            part("corporation", corporation, &mut unavailable)
                .into_iter()
                .map(|offer| ("corporation", offer)),
        );
        if let Some(alliance_id) = affiliation.alliance_id {
            let alliance = esi.alliance_offers(alliance_id).await;
            all_offers.extend(
                part("alliance", alliance, &mut unavailable)
                    .into_iter()
                    .map(|offer| ("alliance", offer)),
            );
        }
    }

    // A listing offered to the character and to their corporation or alliance
    // shows once, as the most personal offer.
    let mut offered = BTreeSet::new();
    all_offers.retain(|(_, offer)| offered.insert(offer.id.clone()));

    // The designs these refer to, fetched once if the hub sync has not.
    let mut mentioned: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    let ids = licenses
        .iter()
        .map(|license| &license.skinr_id)
        .chain(listings.iter().map(|listing| &listing.skinr_id))
        .chain(all_offers.iter().map(|(_, offer)| &offer.skinr_id));
    for id in ids {
        if seen.insert(id.clone()) {
            mentioned.push(id.clone());
        }
    }
    fetch_missing_designs(&state.pool, &state.esi, &mentioned, MAX_NEW_DESIGNS)
        .await
        .map_err(database_error)?;

    let created: Vec<String> = sqlx::query_scalar(
        "select id from skinr_designs where creator_id = $1 order by fetched_at desc, id",
    )
    .bind(character.id)
    .fetch_all(&state.pool)
    .await
    .map_err(database_error)?;
    for id in &created {
        if seen.insert(id.clone()) {
            mentioned.push(id.clone());
        }
    }

    let sellers: Vec<i64> = all_offers
        .iter()
        .map(|(_, offer)| offer.seller_id)
        .collect();
    let creators: Vec<i64> =
        sqlx::query_scalar("select distinct creator_id from skinr_designs where id = any($1)")
            .bind(&mentioned)
            .fetch_all(&state.pool)
            .await
            .map_err(database_error)?;
    name_characters(
        &state.pool,
        &state.esi,
        &[sellers.as_slice(), creators.as_slice()].concat(),
    )
    .await
    .map_err(database_error)?;

    let mut used: BTreeSet<i32> = component_licenses
        .iter()
        .map(|license| license.component_id as i32)
        .collect();
    let designs = load_designs(&state.pool, &mentioned, &mut used)
        .await
        .map_err(database_error)?;
    let components = load_components(&state.pool, &used)
        .await
        .map_err(database_error)?;
    let slot_materials = load_slot_materials(&state.pool)
        .await
        .map_err(database_error)?;

    let seller_names: std::collections::HashMap<i64, String> = sqlx::query_as::<_, (i64, String)>(
        "select id, name from esi_characters where id = any($1)",
    )
    .bind(&sellers)
    .fetch_all(&state.pool)
    .await
    .map_err(database_error)?
    .into_iter()
    .collect();
    let component_licenses = merge_component_licenses(&component_licenses);
    let offers = all_offers
        .into_iter()
        .map(|(audience, offer)| OfferOut {
            audience,
            seller: super::skinr::Person {
                id: offer.seller_id,
                name: seller_names.get(&offer.seller_id).cloned(),
            },
            listing: offer.into(),
        })
        .collect();

    Ok(Json(MySkinr {
        character: CharacterOut {
            id: character.id,
            name: character.name,
            corporation_id: affiliation.as_ref().map(|a| a.corporation_id),
            alliance_id: affiliation.and_then(|a| a.alliance_id),
        },
        designs,
        components,
        slot_materials,
        created,
        licenses,
        component_licenses,
        listings: listings.into_iter().map(Into::into).collect(),
        offers,
        unavailable,
    }))
}

/// The character's access token; a refused refresh means logging in again.
async fn token_for(
    state: &AppState,
    sso: &SsoClient,
    character_id: i64,
) -> Result<String, Response> {
    session::access_token(&state.pool, sso, character_id)
        .await
        .map_err(|cause| match cause {
            TokenError::Revoked => error(
                StatusCode::UNAUTHORIZED,
                "Your EVE login ran out. Log in again.",
            ),
            TokenError::Db(cause) => database_error(cause),
            TokenError::Sso(cause) => {
                tracing::warn!(error = %cause, "could not refresh an access token");
                error(
                    StatusCode::BAD_GATEWAY,
                    "EVE's login is unavailable right now.",
                )
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn license(kind: &str, component_id: i64, runs: &str) -> ComponentLicense {
        serde_json::from_str(&format!(
            r#"{{"component_id": {component_id}, "type": "{kind}", "runs": {runs}}}"#
        ))
        .expect("a license in ESI's shape")
    }

    #[test]
    fn a_component_shows_once_however_many_licenses() {
        let merged = merge_component_licenses(&[
            license("nanocoating", 105, r#"{"unlimited": true}"#),
            license("nanocoating", 105, r#"{"remaining": 6}"#),
            license("pattern", 7, r#"{"remaining": 2}"#),
            license("pattern", 7, r#"{"remaining": 3}"#),
            license("nanocoating", 4, r#"{"remaining": 1}"#),
        ]);
        let shape: Vec<_> = merged
            .iter()
            .map(|out| (out.kind.as_str(), out.component_id, out.runs))
            .collect();

        assert_eq!(
            shape,
            vec![
                ("nanocoating", 4, Some(1)),
                ("nanocoating", 105, None),
                ("pattern", 7, Some(5)),
            ]
        );
    }

    #[test]
    fn a_license_without_a_count_is_unlimited() {
        assert_eq!(license("pattern", 1, "{}").runs.left(), None);
    }

    #[test]
    fn a_bare_count_is_read_too() {
        assert_eq!(license("pattern", 1, "4").runs.left(), Some(4));
    }
}

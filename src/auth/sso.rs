//! EVE SSO (OAuth 2.0 at login.eveonline.com), authorization code flow with
//! the client secret, which never leaves the API: the authorize redirect, the
//! code and refresh grants, token revocation, and the access token's JWT
//! checks (signature against CCP's JWKS, issuer, audience, expiry).
//!
//! Endpoints come from the SSO's metadata document rather than being
//! hard-coded, as CCP asks; it and the JWKS are cached for a few minutes.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use reqwest::{StatusCode, Url};
use serde::Deserialize;

/// Where EVE SSO lives; tests point `EVE_SSO_URL` at a local mock.
const DEFAULT_SSO_URL: &str = "https://login.eveonline.com";
/// The metadata document under the SSO's base URL.
const METADATA_PATH: &str = "/.well-known/oauth-authorization-server";
/// How long the metadata and the JWKS are trusted before they are fetched again.
const METADATA_CACHE: Duration = Duration::from_secs(5 * 60);
/// Issuers an access token may carry (CCP's docs name all three spellings).
const ACCEPTED_ISSUERS: [&str; 3] = [
    "login.eveonline.com",
    "https://login.eveonline.com",
    "https://login.eveonline.com/",
];
/// The audience every EVE access token carries besides the app's client id.
const EVE_AUDIENCE: &str = "EVE Online";
/// The `sub` claim's prefix in front of the character id.
const CHARACTER_SUBJECT: &str = "CHARACTER:EVE:";

#[derive(Debug)]
pub enum Error {
    Http(reqwest::Error),
    /// The SSO answered with an error status.
    Status(StatusCode, String),
    Token(jsonwebtoken::errors::Error),
    /// The token is well formed but not one we accept.
    Rejected(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Http(error) => write!(f, "SSO request failed: {error}"),
            Error::Status(status, body) => write!(f, "SSO answered {status}: {body}"),
            Error::Token(error) => write!(f, "invalid access token: {error}"),
            Error::Rejected(why) => write!(f, "access token rejected: {why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Error::Http(error)
    }
}

impl From<jsonwebtoken::errors::Error> for Error {
    fn from(error: jsonwebtoken::errors::Error) -> Self {
        Error::Token(error)
    }
}

impl Error {
    /// Whether the SSO refused the grant itself (a revoked or spent refresh
    /// token), as opposed to not being reachable.
    pub fn is_refused(&self) -> bool {
        matches!(self, Error::Status(status, _) if status.is_client_error())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Metadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub jwks_uri: String,
    #[serde(default)]
    pub revocation_endpoint: Option<String>,
}

/// What the token endpoint returns.
#[derive(Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    /// Seconds the access token is valid for.
    pub expires_in: i64,
}

impl std::fmt::Debug for Tokens {
    // Tokens are credentials: never print them.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("expires_in", &self.expires_in)
            .finish_non_exhaustive()
    }
}

/// The character an access token was issued for.
#[derive(Debug, Clone)]
pub struct Character {
    pub id: i64,
    pub name: String,
    pub owner_hash: String,
    pub scopes: Vec<String>,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    name: String,
    owner: String,
    #[serde(default, deserialize_with = "one_or_many")]
    scp: Vec<String>,
    #[serde(default, deserialize_with = "one_or_many")]
    aud: Vec<String>,
}

/// A claim that is a string or a list of strings (`scp` with one scope is a string).
fn one_or_many<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match Option::<OneOrMany>::deserialize(deserializer)? {
        None => Vec::new(),
        Some(OneOrMany::One(value)) => vec![value],
        Some(OneOrMany::Many(values)) => values,
    })
}

type Cached<T> = Arc<Mutex<Option<(Instant, T)>>>;

#[derive(Clone)]
pub struct SsoClient {
    http: reqwest::Client,
    base_url: String,
    client_id: String,
    client_secret: String,
    callback_url: String,
    metadata: Cached<Metadata>,
    jwks: Cached<JwkSet>,
}

impl SsoClient {
    pub fn new(base_url: &str, client_id: &str, client_secret: &str, callback_url: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_owned(),
            client_id: client_id.to_owned(),
            client_secret: client_secret.to_owned(),
            callback_url: callback_url.to_owned(),
            metadata: Arc::default(),
            jwks: Arc::default(),
        }
    }

    /// The client `EVE_CLIENT_ID`, `EVE_CLIENT_SECRET` and `EVE_CALLBACK_URL`
    /// describe (`EVE_SSO_URL` overrides the SSO), or `None` while the app is
    /// not registered: logging in is then switched off.
    pub fn from_env() -> Option<Self> {
        let var = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let client_id = var("EVE_CLIENT_ID")?;
        let client_secret = var("EVE_CLIENT_SECRET")?;
        let callback_url = var("EVE_CALLBACK_URL")?;
        let base_url = var("EVE_SSO_URL").unwrap_or_else(|| DEFAULT_SSO_URL.to_owned());
        Some(Self::new(
            &base_url,
            &client_id,
            &client_secret,
            &callback_url,
        ))
    }

    /// Whether the callback is served over HTTPS, so cookies can be `Secure`.
    pub fn is_https(&self) -> bool {
        self.callback_url.starts_with("https://")
    }

    /// Where a login starts: `/eve/login` on the callback's own origin, so
    /// the login cookie is set on the host the SSO sends the browser back to
    /// (a page opened on another name for the same site would lose it).
    pub fn login_url(&self) -> String {
        let after_scheme = self.callback_url.find("://").map_or(0, |at| at + 3);
        let origin_end = self.callback_url[after_scheme..]
            .find('/')
            .map_or(self.callback_url.len(), |at| after_scheme + at);
        format!("{}/eve/login", &self.callback_url[..origin_end])
    }

    pub async fn metadata(&self) -> Result<Metadata, Error> {
        if let Some(metadata) = fresh(&self.metadata) {
            return Ok(metadata);
        }
        let metadata: Metadata = self
            .checked(
                self.http
                    .get(format!("{}{METADATA_PATH}", self.base_url))
                    .send()
                    .await?,
            )
            .await?
            .json()
            .await?;
        *self.metadata.lock().expect("metadata lock") = Some((Instant::now(), metadata.clone()));
        Ok(metadata)
    }

    /// Where to send the browser to log in.
    pub async fn authorize_url(&self, state: &str, scopes: &[&str]) -> Result<String, Error> {
        let metadata = self.metadata().await?;
        let mut url = Url::parse(&metadata.authorization_endpoint)
            .map_err(|_| Error::Rejected("the authorization endpoint is not a URL"))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &self.callback_url)
            .append_pair("client_id", &self.client_id)
            .append_pair("scope", &scopes.join(" "))
            .append_pair("state", state);
        Ok(url.into())
    }

    /// Trades the callback's authorization code for tokens.
    pub async fn exchange_code(&self, code: &str) -> Result<Tokens, Error> {
        self.grant(&[("grant_type", "authorization_code"), ("code", code)])
            .await
    }

    /// A fresh access token for a refresh token. The answer carries a new
    /// refresh token, which replaces the old one.
    pub async fn refresh(&self, refresh_token: &str) -> Result<Tokens, Error> {
        self.grant(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .await
    }

    async fn grant(&self, form: &[(&str, &str)]) -> Result<Tokens, Error> {
        let metadata = self.metadata().await?;
        let response = self
            .http
            .post(&metadata.token_endpoint)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(form)
            .send()
            .await?;
        Ok(self.checked(response).await?.json().await?)
    }

    /// Revokes a refresh token, so it stops working at CCP's end too.
    pub async fn revoke(&self, refresh_token: &str) -> Result<(), Error> {
        let metadata = self.metadata().await?;
        let Some(endpoint) = metadata.revocation_endpoint else {
            return Ok(());
        };
        let response = self
            .http
            .post(endpoint)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[
                ("token_type_hint", "refresh_token"),
                ("token", refresh_token),
            ])
            .send()
            .await?;
        self.checked(response).await?;
        Ok(())
    }

    /// Checks an access token and says whose it is: an RS256 signature by a
    /// key in CCP's JWKS, an accepted issuer, our client id and "EVE Online"
    /// in the audience, not expired, and a character as the subject.
    pub async fn verify(&self, access_token: &str) -> Result<Character, Error> {
        let header = jsonwebtoken::decode_header(access_token)?;
        let kid = header.kid.ok_or(Error::Rejected("no key id"))?;
        let mut jwks = self.jwks(false).await?;
        if jwks.find(&kid).is_none() {
            // CCP may have rotated its keys since we fetched them.
            jwks = self.jwks(true).await?;
        }
        let jwk = jwks.find(&kid).ok_or(Error::Rejected("unknown key id"))?;

        // Only the algorithm CCP signs with; never the one the token names.
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&ACCEPTED_ISSUERS);
        validation.set_audience(&[&self.client_id]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        let claims = jsonwebtoken::decode::<Claims>(
            access_token,
            &DecodingKey::from_jwk(jwk)?,
            &validation,
        )?
        .claims;
        if !claims.aud.iter().any(|aud| aud == EVE_AUDIENCE) {
            return Err(Error::Rejected("not an EVE Online token"));
        }
        let id = claims
            .sub
            .strip_prefix(CHARACTER_SUBJECT)
            .and_then(|id| id.parse().ok())
            .ok_or(Error::Rejected("the subject is not a character"))?;
        Ok(Character {
            id,
            name: claims.name,
            owner_hash: claims.owner,
            scopes: claims.scp,
        })
    }

    async fn jwks(&self, reload: bool) -> Result<JwkSet, Error> {
        if !reload && let Some(jwks) = fresh(&self.jwks) {
            return Ok(jwks);
        }
        let metadata = self.metadata().await?;
        let jwks: JwkSet = self
            .checked(self.http.get(&metadata.jwks_uri).send().await?)
            .await?
            .json()
            .await?;
        *self.jwks.lock().expect("jwks lock") = Some((Instant::now(), jwks.clone()));
        Ok(jwks)
    }

    async fn checked(&self, response: reqwest::Response) -> Result<reqwest::Response, Error> {
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        Err(Error::Status(
            status,
            response.text().await.unwrap_or_default(),
        ))
    }
}

fn fresh<T: Clone>(cached: &Cached<T>) -> Option<T> {
    let cached = cached.lock().expect("cache lock");
    cached
        .as_ref()
        .filter(|(at, _)| at.elapsed() < METADATA_CACHE)
        .map(|(_, value)| value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_login_starts_on_the_callbacks_origin() {
        let sso = |callback| SsoClient::new("https://login.example", "id", "secret", callback);
        assert_eq!(
            sso("https://skinr-space.test/eve/callback").login_url(),
            "https://skinr-space.test/eve/login"
        );
        assert_eq!(
            sso("http://localhost:5200/eve/callback").login_url(),
            "http://localhost:5200/eve/login"
        );
    }
}

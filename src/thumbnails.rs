//! Keeps every ship's thumbnails current. The render worker
//! (`frontend/scripts/render-server.ts`) draws them; this decides which and
//! writes what it answers into the thumbnail directory, which
//! [`crate::server::thumbnails`] serves. Per [`Subject`]:
//!
//! - `<stem>-dark.png`, `<stem>-light.png`: transparent, [`SIZE`] px square
//! - `<stem>.files`: every `res:/` path the render used, as a JSON array,
//!   which the page preloads ([`crate::server::ship_files`])
//! - `<stem>.dna`: the DNA they show, written last
//!
//! A pass goes through every ship, every SKINR design on sale, every in-game
//! skin, then the other designs the API knows, and draws only those whose
//! DNA (a design's look) differs from their `.dna` (after an SDE import, or
//! new ones) or whose files are incomplete, one at a time, so a pass that
//! stops halfway just carries on next time. The debug tools
//! (`server::debug`) draw one on request; the Open Graph cards show a skin's
//! or design's while it is current ([`current_png`]), and the skin cards
//! beside the skin's icon.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use futures_util::StreamExt as _;
use serde::Deserialize;
use sqlx::PgPool;

use crate::dna;
use crate::esi::skinr::LISTED;

/// Width and height of a thumbnail, in px: a card at 2x.
pub const SIZE: u32 = 512;
/// The themes a thumbnail is drawn in, as the worker names them.
pub const THEMES: [&str; 2] = ["dark", "light"];
/// The folder a skin's thumbnails go in, inside the thumbnail directory.
pub const SKIN_FOLDER: &str = "skins";
/// The folder a SKINR design's thumbnails go in, inside the thumbnail directory.
pub const DESIGN_FOLDER: &str = "designs";
/// Time between the end of one pass and the start of the next.
const PASS_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// How soon to try again when the worker could not be reached (it may be
/// starting, and a cold one downloads the game's SOF first).
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
/// How long one ship may take, queue included (lavapipe draws on the CPU,
/// and the cards' renders share the worker's queue).
const DRAW_TIMEOUT: Duration = Duration::from_secs(600);
/// A long pass (the first one draws thousands on the CPU) says how far it
/// got after every this many drawings.
const PROGRESS_EVERY: usize = 100;
/// The first bytes of every PNG.
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// What a thumbnail shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// A ship in its default livery, by type id.
    Ship(i32),
    /// One of a ship's in-game skins, by type and skin id (a skin can fit
    /// several hulls).
    Skin { ship: i32, skin: i32 },
    /// A SKINR design (its 64 hex digit id) on its ship, by type id.
    Design { id: String, ship: i32 },
}

impl Subject {
    /// Its files' path in the thumbnail directory, before `-<theme>.png` or
    /// `.dna`: `587`, `skins/587_1234` or `designs/<id>`.
    pub fn stem(&self) -> String {
        match self {
            Self::Ship(id) => id.to_string(),
            Self::Skin { ship, skin } => format!("{SKIN_FOLDER}/{ship}_{skin}"),
            Self::Design { id, .. } => format!("{DESIGN_FOLDER}/{id}"),
        }
    }

    /// What the worker is asked to draw. A ship or skin is its DNA, `key`;
    /// a design the worker looks up and turns into DNA itself, and `key` is
    /// only its look ([`crate::dna::design_look`]).
    fn request(&self, key: &str) -> serde_json::Value {
        match self {
            Self::Ship(_) | Self::Skin { .. } => serde_json::json!({ "dna": key, "size": SIZE }),
            Self::Design { id, ship } => {
                serde_json::json!({ "design": id, "ship": ship, "size": SIZE })
            }
        }
    }
}

/// A subject's thumbnails on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    /// The DNA (a design's look) they were drawn from, if any were.
    pub drawn_from: Option<String>,
    /// When each theme's PNG was written, in [`THEMES`] order.
    pub written: [Option<SystemTime>; 2],
}

/// The thumbnail directory and the worker that draws into it.
#[derive(Clone)]
pub struct Thumbnails {
    pool: PgPool,
    dir: PathBuf,
    /// The worker's base URL, without a trailing slash.
    renderer: String,
    http: reqwest::Client,
    /// How many drawings a pass keeps in flight: the worker's `WORKERS`.
    concurrency: usize,
}

/// What a pass did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    /// Ships, skins and designs looked at.
    pub checked: usize,
    pub drawn: usize,
    pub failed: usize,
}

/// Why a pass stopped early.
#[derive(Debug)]
pub enum PassError {
    Database(sqlx::Error),
    /// The worker did not answer at all; the rest of the pass would not either.
    Unreachable(reqwest::Error),
}

impl fmt::Display for PassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "reading the ships: {error}"),
            Self::Unreachable(error) => write!(f, "render worker unreachable: {error}"),
        }
    }
}

impl std::error::Error for PassError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Unreachable(error) => Some(error),
        }
    }
}

/// Why one subject was not drawn.
#[derive(Debug)]
pub enum DrawError {
    Unreachable(reqwest::Error),
    /// The worker answered, but not with a thumbnail.
    Refused(String),
    Write(std::io::Error),
}

impl fmt::Display for DrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreachable(error) => write!(f, "render worker unreachable: {error}"),
            Self::Refused(message) => write!(f, "{message}"),
            Self::Write(error) => write!(f, "writing: {error}"),
        }
    }
}

impl std::error::Error for DrawError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreachable(error) => Some(error),
            Self::Refused(_) => None,
            Self::Write(error) => Some(error),
        }
    }
}

/// The worker's answer: a base64 PNG per theme, and the paths the ship used.
#[derive(Deserialize)]
struct ThumbnailAnswer {
    pngs: std::collections::HashMap<String, String>,
    used: Vec<String>,
    missing: Vec<String>,
}

/// A livery to keep a thumbnail of, as the database has it.
#[derive(sqlx::FromRow)]
struct LiveryRow {
    ship: i32,
    /// The skin's id, or a design's; `None` for a ship's own livery.
    skin: Option<i32>,
    design: Option<String>,
    sof_hull: String,
    sof_faction: String,
    sof_race: String,
    /// The material set's commands, or a design's layout.
    extra: String,
}

impl LiveryRow {
    /// What to draw, and what its drawing is marked with.
    fn subject(self) -> (Subject, String) {
        let Self {
            ship,
            skin,
            design,
            sof_hull,
            sof_faction,
            sof_race,
            extra,
        } = self;
        if let Some(id) = design {
            let look = dna::design_look(&sof_hull, &sof_faction, &sof_race, &extra);
            return (Subject::Design { id, ship }, look);
        }
        let dna = dna::livery(&sof_hull, &sof_faction, &sof_race, &extra);
        let subject = skin.map_or(Subject::Ship(ship), |skin| Subject::Skin { ship, skin });
        (subject, dna)
    }
}

/// Every livery a pass keeps current, in the order it draws them: the ships
/// (the explorer shows them all), the designs on sale (the Paragon Hub shows
/// them), the in-game skins, then the designs no longer on sale.
const LIVERIES: &str = "
    select id as ship, null::integer as skin, null::text as design,
           sof_hull, sof_faction, sof_race, sof_commands as extra, 0 as stage
    from ships
    union all
    select k.ship_id, k.skin_id, null, s.sof_hull, k.sof_faction, s.sof_race, k.sof_commands, 2
    from ship_skins k join ships s on s.id = k.ship_id
    union all
    select d.ship_type_id, null, d.id, s.sof_hull, s.sof_faction, s.sof_race, d.layout,
           case when exists (select 1 from skinr_listings l
                             where l.skinr_id = d.id and l.state = $1 and l.expires > now())
                then 1 else 3 end
    from skinr_designs d join ships s on s.id = d.ship_type_id
    order by stage, ship, skin, design";

impl Thumbnails {
    pub fn new(pool: PgPool, dir: PathBuf, renderer: &str) -> Self {
        Self {
            pool,
            dir,
            renderer: renderer.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .timeout(DRAW_TIMEOUT)
                .build()
                .expect("a reqwest client builds"),
            concurrency: 1,
        }
    }

    /// Lets a pass keep `concurrency` drawings in flight (at least one): as
    /// many as the worker has processes.
    #[must_use]
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    fn path(&self, subject: &Subject, ending: &str) -> PathBuf {
        path(&self.dir, subject, ending)
    }

    /// Draws every ship, skin and design whose thumbnails do not show its
    /// current DNA (or look).
    ///
    /// # Errors
    ///
    /// When the liveries cannot be read, or the worker cannot be reached; one
    /// the worker fails to draw only counts as failed.
    pub async fn pass(&self) -> Result<Report, PassError> {
        let liveries = sqlx::query_as::<_, LiveryRow>(LIVERIES)
            .bind(LISTED)
            .fetch_all(&self.pool)
            .await
            .map_err(PassError::Database)?;
        let mut report = Report {
            checked: liveries.len(),
            ..Report::default()
        };
        // In order, a few at a time; a livery that is current is skipped
        // without taking a place.
        let mut outcomes = futures_util::stream::iter(liveries)
            .map(|livery| async move {
                let (subject, key) = livery.subject();
                if self.current(&subject, &key).await {
                    return None;
                }
                let drawn = self.draw(&subject, &key).await;
                Some((subject, key, drawn))
            })
            .buffer_unordered(self.concurrency);
        while let Some(outcome) = outcomes.next().await {
            let Some((subject, key, drawn)) = outcome else {
                continue;
            };
            match drawn {
                Ok(()) => {
                    report.drawn += 1;
                    if report.drawn.is_multiple_of(PROGRESS_EVERY) {
                        tracing::info!(?report, "thumbnails pass under way");
                    }
                }
                Err(DrawError::Unreachable(error)) => return Err(PassError::Unreachable(error)),
                Err(error) => {
                    tracing::warn!(?subject, %key, %error, "thumbnail failed");
                    report.failed += 1;
                }
            }
        }
        Ok(report)
    }

    /// What is on disk for `subject`.
    pub async fn status(&self, subject: &Subject) -> Status {
        let drawn_from = tokio::fs::read_to_string(self.path(subject, ".dna"))
            .await
            .ok();
        let mut written = [None; 2];
        for (slot, theme) in written.iter_mut().zip(THEMES) {
            *slot = tokio::fs::metadata(self.path(subject, &format!("-{theme}.png")))
                .await
                .and_then(|meta| meta.modified())
                .ok();
        }
        Status {
            drawn_from,
            written,
        }
    }

    /// Whether `subject` has all its files, drawn from `dna`.
    async fn current(&self, subject: &Subject, dna: &str) -> bool {
        let status = self.status(subject).await;
        status.drawn_from.as_deref() == Some(dna)
            && status.written.iter().all(Option::is_some)
            && tokio::fs::metadata(self.path(subject, ".files"))
                .await
                .is_ok()
    }

    /// Asks the worker for `subject` and writes what it draws, marked with
    /// `key` (its DNA, or a design's look).
    ///
    /// # Errors
    ///
    /// When the worker cannot be reached or draws nothing, or the files
    /// cannot be written; nothing is marked drawn then.
    pub async fn draw(&self, subject: &Subject, key: &str) -> Result<(), DrawError> {
        let response = self
            .http
            .post(format!("{}/thumbnail", self.renderer))
            .json(&subject.request(key))
            .send()
            .await
            .map_err(|error| {
                if error.is_connect() {
                    DrawError::Unreachable(error)
                } else {
                    DrawError::Refused(error.to_string())
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(DrawError::Refused(format!("{status}: {message}")));
        }
        let answer: ThumbnailAnswer = response
            .json()
            .await
            .map_err(|error| DrawError::Refused(format!("not a thumbnail: {error}")))?;
        let mut pngs = Vec::with_capacity(THEMES.len());
        for theme in THEMES {
            let png = answer
                .pngs
                .get(theme)
                .and_then(|encoded| BASE64.decode(encoded).ok())
                .filter(|png| png.starts_with(PNG_SIGNATURE))
                .ok_or_else(|| DrawError::Refused(format!("no {theme} PNG")))?;
            pngs.push((theme, png));
        }
        if !answer.missing.is_empty() {
            tracing::info!(?subject, missing = ?answer.missing, "thumbnail drawn without some files");
        }
        let used = serde_json::to_vec(&answer.used).expect("a list of strings serializes");
        for (theme, png) in pngs {
            write(&self.path(subject, &format!("-{theme}.png")), &png).await?;
        }
        write(&self.path(subject, ".files"), &used).await?;
        // Last: thumbnails without it are drawn again next time.
        write(&self.path(subject, ".dna"), key.as_bytes()).await?;
        tracing::debug!(?subject, %key, "thumbnail drawn");
        Ok(())
    }
}

fn path(dir: &Path, subject: &Subject, ending: &str) -> PathBuf {
    dir.join(format!("{}{ending}", subject.stem()))
}

/// `subject`'s dark thumbnail in `dir`, if it was drawn from `key` (its DNA,
/// or a design's look): what a card shows while its own render is missing.
pub async fn current_png(dir: &Path, subject: &Subject, key: &str) -> Option<Vec<u8>> {
    let drawn_from = tokio::fs::read_to_string(path(dir, subject, ".dna"))
        .await
        .ok()?;
    if drawn_from != key {
        return None;
    }
    tokio::fs::read(path(dir, subject, &format!("-{}.png", THEMES[0])))
        .await
        .ok()
}

/// Writes a file aside and moves it into place, so a reader never sees half of it.
async fn write(path: &Path, bytes: &[u8]) -> Result<(), DrawError> {
    let part = path.with_extension(format!("{}.part", std::process::id()));
    let written = async {
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(&part, bytes).await?;
        tokio::fs::rename(&part, path).await
    }
    .await;
    if written.is_err() {
        tokio::fs::remove_file(&part).await.ok();
    }
    written.map_err(DrawError::Write)
}

/// Runs [`Thumbnails::pass`] forever: at start (after an SDE import the API
/// starts again), then [`PASS_INTERVAL`] after each pass, or sooner when the
/// worker was not reachable.
pub fn spawn(thumbnails: Thumbnails) {
    tokio::spawn(async move {
        loop {
            let wait = match thumbnails.pass().await {
                Ok(report) => {
                    if report.drawn > 0 || report.failed > 0 {
                        tracing::info!(?report, "thumbnails pass finished");
                    }
                    PASS_INTERVAL
                }
                Err(error @ PassError::Unreachable(_)) => {
                    tracing::info!(%error, "thumbnails: trying again shortly");
                    RETRY_INTERVAL
                }
                Err(error) => {
                    tracing::warn!(%error, "thumbnails pass failed");
                    PASS_INTERVAL
                }
            };
            tokio::time::sleep(wait).await;
        }
    });
}

//! Imports the ship reference data from the SDE into Postgres.
//!
//! Usage: `cargo run --bin sde_import`
//! The build-versioned SDE zip is cached in `storage/sde/`; delete the
//! directory to force a fresh download. When the database already carries
//! the latest SDE build the import is skipped (the docker bootstrap runs this
//! on every `up`); set `SDE_FORCE=1` to reseed anyway.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use futures_util::StreamExt;

use skinr::db;
use skinr::sde::client::{ClientResources, Error, REQUIRED_FILES, SdeClient, extract_files};
use skinr::sde::{logos, parse, seed};

/// Where the client keeps an icon per skin material, named by material id.
const SKIN_ICON_DIR: &str = "res:/ui/texture/classes/skins/icons";
/// Where the client keeps the greyscale layers of a skin icon.
const SKIN_ICON_LAYER_DIR: &str = "res:/ui/texture/classes/skins";
/// Those layers, bottom to top.
const SKIN_ICON_LAYERS: [&str; 4] = ["main", "secondary", "hull", "lights"];
/// How many skin icons are fetched from the CDN at once.
const SKIN_ICON_FETCHES: usize = 16;

/// Where the downloaded zip and its extracted files are kept.
const STORAGE_DIR: &str = "storage/sde";

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Local configuration from .env, if present; real environment wins.
    dotenvy::dotenv().ok();

    let storage = Path::new(STORAGE_DIR);
    std::fs::create_dir_all(storage)?;

    let client = SdeClient::default();
    let build = client.latest_build_number().await?;
    println!("latest SDE build: {build}");

    let pool = db::connect().await?;
    db::migrate(&pool).await?;

    let force = std::env::var("SDE_FORCE").is_ok_and(|value| value == "1" || value == "true");
    if !force && db::seeded_sde_build(&pool).await? == Some(build.to_string()) {
        println!("SDE build {build} already seeded, skipping (SDE_FORCE=1 to reseed)");
        return Ok(());
    }

    let zip_path = storage.join(format!("eve-online-static-data-{build}-jsonl.zip"));
    if zip_path.exists() {
        println!("using cached {}", zip_path.display());
    } else {
        println!("downloading {}...", zip_path.display());
        client.download_data(build, &zip_path).await?;
    }

    let extracted = extract_files(&zip_path, &REQUIRED_FILES, storage)?;
    let open = |index: usize| -> Result<BufReader<File>, Error> {
        Ok(BufReader::new(File::open(&extracted[index])?))
    };

    let tables = parse::parse(parse::SdeFiles {
        types: open(0)?,
        groups: open(1)?,
        graphics: open(2)?,
        races: open(3)?,
        meta_groups: open(4)?,
        skins: open(5)?,
        skin_materials: open(6)?,
        material_sets: open(7)?,
        factions: open(8)?,
        type_dogma: open(9)?,
        dogma_attributes: open(10)?,
        dogma_units: open(11)?,
        type_bonus: open(12)?,
        icons: open(13)?,
    })?;
    println!(
        "parsed {} ships in {} groups, {} skins, {} attribute values, {} bonus lines",
        tables.ships.len(),
        tables.groups.len(),
        tables.skins.len(),
        tables.attributes.len(),
        tables.bonuses.len()
    );

    seed::seed(&pool, &tables).await?;
    db::record_sde_build(&pool, &build.to_string()).await?;
    println!("seeded Postgres (build {build})");

    // Faction emblems come from the game client, not the SDE. A failure here
    // leaves the seeded data alone; ship pages then simply show no emblem.
    let names: Vec<String> = tables
        .factions
        .iter()
        .filter_map(|faction| faction.logo.clone())
        .collect();
    let resources = match ClientResources::load().await {
        Ok(resources) => resources,
        Err(error) => {
            println!("could not read the client file index: {error}");
            return Ok(());
        }
    };
    match logos::fetch(&resources, &names).await {
        Ok(fetched) => {
            seed::store_logos(&pool, &fetched).await?;
            println!("stored {} of {} faction logos", fetched.len(), names.len());
        }
        Err(error) => println!("could not fetch faction logos: {error}"),
    }

    // The client's own icon for each spec sheet attribute (already PNG).
    let mut icons = Vec::new();
    for (name, path) in &tables.attribute_icons {
        match resources.read(path).await {
            Ok(Some(png)) => icons.push((name.clone(), png)),
            Ok(None) => println!("the client has no {path} for {name}"),
            Err(error) => println!("could not fetch {path}: {error}"),
        }
    }
    seed::store_attribute_icons(&pool, &icons).await?;
    println!(
        "stored {} of {} attribute icons",
        icons.len(),
        tables.attribute_icons.len()
    );

    // The client's icon for each skin material in use.
    let materials: std::collections::BTreeSet<i32> =
        tables.skins.iter().map(|skin| skin.material_id).collect();
    let fetched: Vec<_> = futures_util::stream::iter(materials.iter().copied())
        .map(|material| {
            let resources = &resources;
            async move {
                let path = format!("{SKIN_ICON_DIR}/{material}.png");
                (material, resources.read(&path).await)
            }
        })
        .buffer_unordered(SKIN_ICON_FETCHES)
        .collect()
        .await;
    let mut skin_icons = Vec::new();
    for (material, result) in fetched {
        match result {
            Ok(Some(png)) => skin_icons.push((material, png)),
            Ok(None) => {}
            Err(error) => println!("could not fetch the icon of skin material {material}: {error}"),
        }
    }
    seed::store_skin_icons(&pool, &skin_icons).await?;
    println!(
        "stored {} of {} skin material icons",
        skin_icons.len(),
        materials.len()
    );

    // The layers the client tints into an icon for a livery without one.
    let mut layers = Vec::new();
    for name in SKIN_ICON_LAYERS {
        let path = format!("{SKIN_ICON_LAYER_DIR}/skin-icon-{name}.png");
        match resources.read(&path).await {
            Ok(Some(png)) => layers.push((name, png)),
            Ok(None) => println!("the client has no {path}"),
            Err(error) => println!("could not fetch {path}: {error}"),
        }
    }
    seed::store_skin_icon_layers(&pool, &layers).await?;
    println!(
        "stored {} of {} skin icon layers",
        layers.len(),
        SKIN_ICON_LAYERS.len()
    );

    Ok(())
}

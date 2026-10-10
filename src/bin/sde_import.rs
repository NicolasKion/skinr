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
use skinr::sde::skin_icon_colors::{self, Rgba};
use skinr::sde::skinr::swatch_color as ship_skinr_swatch;
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
        skinr_components: open(14)?,
        skinr_component_categories: open(15)?,
        skinr_slots_to_materials: open(16)?,
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

    // Each skin's colors read back off its icon, so the frontend can draw the
    // icon as vectors instead of the 64 px image.
    let decoded: Option<Vec<Rgba>> = skin_icon_colors::LAYERS
        .iter()
        .map(|name| {
            let (_, png) = layers.iter().find(|(layer, _)| layer == name)?;
            Rgba::decode(png)
        })
        .collect();
    if let Some(Ok(decoded)) = decoded.map(<[Rgba; 4]>::try_from) {
        let colors: Vec<(i32, String)> = skin_icons
            .iter()
            .filter_map(|(material, png)| {
                let colors = skin_icon_colors::icon_colors(&Rgba::decode(png)?, &decoded)?;
                Some((*material, serde_json::to_string(&colors).ok()?))
            })
            .collect();
        seed::store_skin_icon_colors(&pool, &colors).await?;
        println!(
            "read colors off {} of {} skin icons",
            colors.len(),
            skin_icons.len()
        );
    }

    // The client's icon for each SKINR component, as the in-game designer shows it.
    let components: Vec<(i32, String)> = tables
        .skinr_components
        .iter()
        .filter_map(|component| Some((component.id, component.icon_file.clone()?)))
        .collect();
    let fetched: Vec<_> = futures_util::stream::iter(components.iter().cloned())
        .map(|(id, path)| {
            let resources = &resources;
            async move { (id, path.clone(), resources.read(&path).await) }
        })
        .buffer_unordered(SKIN_ICON_FETCHES)
        .collect()
        .await;
    let mut component_icons = Vec::new();
    for (id, path, result) in fetched {
        match result {
            Ok(Some(png)) => component_icons.push((id, png)),
            Ok(None) => {}
            Err(error) => println!("could not fetch {path}: {error}"),
        }
    }
    seed::store_skinr_component_icons(&pool, &component_icons).await?;
    // Materials get a swatch color from their icon, so designs can be drawn
    // as the game's SKIN hexagon; patterns are masks and have none.
    let patterns: std::collections::HashSet<i32> = tables
        .skinr_components
        .iter()
        .filter(|component| component.category == "Pattern")
        .map(|component| component.id)
        .collect();
    let colors: Vec<(i32, String)> = component_icons
        .iter()
        .filter(|(id, _)| !patterns.contains(id))
        .filter_map(|(id, png)| Some((*id, ship_skinr_swatch(png)?)))
        .collect();
    seed::store_skinr_component_colors(&pool, &colors).await?;
    println!("sampled {} SKINR material colors", colors.len());
    println!(
        "stored {} of {} SKINR component icons",
        component_icons.len(),
        components.len()
    );

    Ok(())
}

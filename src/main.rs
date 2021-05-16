use color_eyre::{eyre::Report, eyre::WrapErr, Section};
use tracing::{info, instrument};

use cratesio_dbdump_csvtab::{CratesIODumpLoader, rusqlite::{self, Connection}};

#[instrument]
fn main() -> Result<(), Report> {
    #[cfg(feature = "capture-spantrace")]
    install_tracing();

    color_eyre::install()?;

    // Load dump from a .tar.gz archive.
    let db = CratesIODumpLoader::default()
        .minimal()
        .preload(true)
        .update()?
        .open_db()?;

    let bevy_crates = get_bevy_crates(&db)?;

    info!("Bevy's crate id: {:?}", &bevy_crates);

    
    let mut main_crate_metadata = db.prepare(
        r#"
        SELECT crates.id, crates.name, versions.license, versions.num, versions.id 
        FROM versions LEFT JOIN crates 
            ON versions.crate_id = crates.id 
            WHERE crates.name = ?
    "#,
    )?;
    let row = main_crate_metadata.query_map(
        // [vec!["bevy_prototype_lyon", "bevy_egui", "bevy_webgl2"]],
        ["bevy_egui"],
        |r| {
            Ok((
                r.get_unwrap::<_, String>(0),
                r.get_unwrap::<_, String>(1),
                r.get_unwrap::<_, String>(2),
                r.get_unwrap::<_, String>(3),
                get_bevy_versions_for_crate(&db, &r.get_unwrap::<_, String>(0), &r.get_unwrap::<_, String>(4), &bevy_crates)
            ))
        },
    )?;
    info!("{:?}", row.collect::<Vec<_>>());
    Ok(())
}

#[instrument]
fn get_bevy_crates(db: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s = db.prepare_cached("SELECT id, name FROM crates WHERE homepage = ? AND repository = ?")?;
    let rows = s.query_and_then(
        ["https://bevyengine.org", "https://github.com/bevyengine/bevy"], 
        |r| -> Result<(String, String), rusqlite::Error> {
            Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
        })?;
    let mut bevy_crates = Vec::new();
    for bevy_crate in rows {
        bevy_crates.push(bevy_crate?);
    }
    Ok(bevy_crates)
}

#[instrument]
fn get_bevy_versions_for_crate(db: &Connection, cid: &str, vid: &str, bevy_crates: &Vec<(String, String)>) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s = db.prepare_cached(
        r#"
        SELECT dependencies.req
        FROM dependencies LEFT JOIN versions
            ON dependencies.version_id = versions.id 
            WHERE versions.crate_id = ? AND versions.id = ? AND dependencies.crate_id = ?
    "#,
    )?;
    let mvbv: Vec<(String, String)> = bevy_crates.iter().map(|(bid, name)| -> Result<(String, String), rusqlite::Error> {
        Ok((s.query_row([cid,vid,bid], |r| r.get::<_, String>(0))?, name.to_string()))
    }).filter_map(|f| f.ok()).collect();
    Ok(mvbv)
}

#[cfg(feature = "capture-spantrace")]
fn install_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    let fmt_layer = fmt::layer().with_target(false);
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

#[instrument]
fn read_file(path: &str) -> Result<(), Report> {
    info!("Reading file");
    Ok(std::fs::read_to_string(path).map(drop)?)
}

#[instrument]
fn read_config() -> Result<(), Report> {
    read_file("fake_file")
        .wrap_err("Unable to read config")
        .suggestion("try using a file that eists net time")
}

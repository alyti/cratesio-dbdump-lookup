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
        
    info!("Get crate by name: {:?}", get_crate_by_name(&db, "bevy")?);
    info!("Get Latest with bevy as crate: {:?}", get_latest(&db, "bevy".to_string())?);
    info!("Get all of the latest version's dependencies: {:?}", get_latest_dependencies(&db, "bevy".to_string(),DependencyType::All)?);
    info!("Get all of the latest version's dev-dependencies: {:?}", get_latest_dependencies(&db, "bevy".to_string(),DependencyType::Dev)?);
    info!("Get all of the latest version's normal dependencies: {:?}", get_latest_dependencies(&db, "bevy".to_string(),DependencyType::Normal)?);

    info!("Get bevy plugins (based on suffix): {:?}", get_bevy_plugins(&db)?);

    info!("Get rev dependencies: {:?}", get_rev_dependencies(&db, "bevy_egui")?);

    Ok(())
}

//Takes  : crate_name 
//Returns: version_id,version number
fn get_latest(db: &Connection, crate_name: String) -> Result<Vec<(String, String)>, rusqlite::Error>{
    let mut stmt = db.prepare_cached(
        r#"
        SELECT versions.id, versions.num
        FROM versions 
        LEFT JOIN crates
            ON versions.crate_id = crates.id 
        WHERE crates.name = ?
        ORDER BY versions.num DESC
        LIMIT 1
    "#,)?;

    let latest_crates = stmt.query_and_then(
    [crate_name],
    |r| -> Result<(String, String), rusqlite::Error> {
        Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
    })?;
    let mut latest = Vec::new();
    for l_crate in latest_crates {
        latest.push(l_crate?);
    }
    Ok(latest)
}

fn get_latest_dependencies(db: &Connection, crate_name: String, d_type: DependencyType) -> Result<Vec<(String, String)>, rusqlite::Error>{
    let latest = get_latest(db, crate_name)?;
    let version_id: &str = latest.last().unwrap().0.as_ref();
    get_dependencies(db,version_id, d_type)
}


#[derive(PartialEq)]
enum DependencyType {
    Normal,
    Dev,
    All,
}

//Takes  : version.id 
//Returns: dependencies.id, crates.name
fn get_dependencies(db: &Connection, version_id: &str, d_type: DependencyType) -> Result<Vec<(String, String)>, rusqlite::Error>{
    let sql_str = r#"
        SELECT dependencies.id, crates.name
        FROM dependencies 
        LEFT JOIN crates
            ON dependencies.crate_id = crates.id 
        WHERE dependencies.version_id = ?
    "#;
    let mut dependencies = Vec::new();
    if d_type == DependencyType::All {
        let mut stmt = db.prepare_cached(sql_str,
        )?;
        let rows = stmt.query_and_then(
        [version_id],
        |r| -> Result<(String, String), rusqlite::Error> {
            Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
        })?;
        for dependency in rows {
            dependencies.push(dependency?);
        }
    } else {
        let s = sql_str.to_string() + "AND dependencies.kind == ?";
        let mut stmt = db.prepare_cached(&s,
        )?;
        let rows = stmt.query_and_then(
        [version_id, match d_type {
            DependencyType::Normal => 0,
            DependencyType::Dev => 2,
            DependencyType::All => unreachable!(),
        }.to_string().as_ref()],
        |r| -> Result<(String, String), rusqlite::Error> {
            Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
        })?;
        for dependency in rows {
            dependencies.push(dependency?);
        }
    };
    Ok(dependencies)
}

//Returns: dependencies.id, crates.name
fn get_bevy_plugins(db: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    /*let latest = get_latest(db,"bevy".to_string())?;
    let version_id: &str = latest.last().unwrap().0.as_ref();
    let ds = get_dependencies(db,version_id,DependencyType::Normal)?;
    let d_id: &str = ds.last().unwrap().0.as_ref();*/

    let mut s = db.prepare_cached(
        r#"
        SELECT dependencies.id, crates.name
        FROM dependencies 
        LEFT JOIN crates
            ON dependencies.crate_id = crates.id 
        WHERE crates.name like 'bevy_%'
        GROUP BY crates.name
    "#,
    )?;

    let rows = s.query_and_then(
    [],
    |r| -> Result<(String, String), rusqlite::Error> {
        Ok((
            r.get_unwrap::<_, String>(0), 
            r.get_unwrap::<_, String>(1))
        )
    })?;
    let mut bevy_crates = Vec::new();
    for bevy_crate in rows {
        bevy_crates.push(bevy_crate?);
    }
    info!("{}",bevy_crates.len());
    Ok(bevy_crates)
    //Ok(lat_deps)
}

//Takes  : crate.name 
//Returns: crate.id, crates.name
fn get_crate_by_name(db: &Connection, crate_name: &str) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s = db.prepare_cached("SELECT id, name FROM crates WHERE name = ?")?;
    let rows = s.query_and_then(
        [crate_name], 
        |r| -> Result<(String, String), rusqlite::Error> {
            Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
        })?;
    let mut bevy_crates = Vec::new();
    for bevy_crate in rows {
        bevy_crates.push(bevy_crate?);
    }
    Ok(bevy_crates)
}

fn get_rev_dependencies(db: &Connection, crate_name: &str) -> Result<Vec<Result<(String, String, String, String, Result<Vec<(String, String)>, cratesio_dbdump_csvtab::rusqlite::Error>), cratesio_dbdump_csvtab::rusqlite::Error>>, cratesio_dbdump_csvtab::rusqlite::Error>{
    let bevy_crates = get_bevy_crates(&db)?;
    let mut main_crate_metadata = db.prepare(
        r#"
        SELECT crates.id, crates.name, versions.license, versions.num, versions.id 
        FROM versions LEFT JOIN crates 
        ON versions.crate_id = crates.id 
        WHERE crates.name = ?
        ORDER BY versions.num DESC
        LIMIT 1
        "#,
    )?;
    let row = main_crate_metadata.query_map(
        // [vec!["bevy_prototype_lyon", "bevy_egui", "bevy_webgl2"]],
        [crate_name],
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
    Ok(row.collect::<Vec<_>>())
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
        .suggestion("Double-check that the file exist in the current path")
}

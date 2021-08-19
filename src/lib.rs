use color_eyre::{eyre::Report, eyre::WrapErr, Section};
use cratesio_dbdump_csvtab::rusqlite::{self, Connection, Error};
use tracing::{info, instrument};

//Takes  : crate_name
//Returns: version_id,version number

#[instrument]
pub fn get_latest(
    db: &Connection, crate_name: String,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut stmt = db.prepare_cached(
        r#"
        SELECT versions.id, versions.num
        FROM versions 
        LEFT JOIN crates
            ON versions.crate_id = crates.id 
        WHERE crates.name = ?
        ORDER BY versions.num DESC
        LIMIT 1
    "#,
    )?;

    let latest_crates = stmt.query_and_then(
        [crate_name],
        |r| -> Result<(String, String), rusqlite::Error> {
            Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
        },
    )?;
    let mut latest = Vec::new();
    for l_crate in latest_crates {
        latest.push(l_crate?);
    }
    Ok(latest)
}

#[instrument]
pub fn get_latest_dependencies(
    db: &Connection, crate_name: String, d_type: DependencyType,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let latest = get_latest(db, crate_name)?;
    let version_id: &str = latest.last().unwrap().0.as_ref();
    get_dependencies(db, version_id, d_type)
}

#[derive(PartialEq, Debug)]
pub enum DependencyType {
    Normal,
    Dev,
    All,
}

//Takes  : version.id
//Returns: dependencies.id, crates.name
#[instrument]
pub fn get_dependencies(
    db: &Connection, version_id: &str, d_type: DependencyType,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let sql_str = r#"
        SELECT dependencies.id, crates.name
        FROM dependencies 
        LEFT JOIN crates
            ON dependencies.crate_id = crates.id 
        WHERE dependencies.version_id = ?
    "#;
    let mut dependencies = Vec::new();
    if d_type == DependencyType::All {
        let mut stmt = db.prepare_cached(sql_str)?;
        let rows = stmt.query_and_then(
            [version_id],
            |r| -> Result<(String, String), rusqlite::Error> {
                Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
            },
        )?;
        for dependency in rows {
            dependencies.push(dependency?);
        }
    } else {
        let s = sql_str.to_string() + "AND dependencies.kind == ?";
        let mut stmt = db.prepare_cached(&s)?;
        let rows = stmt.query_and_then(
            [
                version_id,
                match d_type {
                    DependencyType::Normal => 0,
                    DependencyType::Dev => 2,
                    DependencyType::All => unreachable!(),
                }
                .to_string()
                .as_ref(),
            ],
            |r| -> Result<(String, String), rusqlite::Error> {
                Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
            },
        )?;
        for dependency in rows {
            dependencies.push(dependency?);
        }
    };
    Ok(dependencies)
}

//Returns: dependencies.id, crates.name
#[instrument]
pub fn get_bevy_plugins_naive(db: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s = db.prepare_cached(
        r#"
        SELECT dependencies.id, crates.name
        FROM dependencies 
        LEFT JOIN crates
            ON dependencies.crate_id = crates.id 
        WHERE crates.name like '%bevy%'
        GROUP BY crates.name
    "#,
    )?;

    let rows = s.query_and_then([], |r| -> Result<(String, String), rusqlite::Error> {
        Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
    })?;
    let mut bevy_crates = Vec::new();
    for bevy_crate in rows {
        bevy_crates.push(bevy_crate?);
    }
    Ok(bevy_crates)
}

//Takes  : crate.name
//Returns: crate.id, crates.name
#[instrument]
pub fn get_crate_by_name(
    db: &Connection, crate_name: &str,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s = db.prepare_cached("SELECT id, name FROM crates WHERE name = ?")?;
    let rows = s.query_and_then(
        [crate_name],
        |r| -> Result<(String, String), rusqlite::Error> {
            Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
        },
    )?;
    let mut bevy_crates = Vec::new();
    for bevy_crate in rows {
        bevy_crates.push(bevy_crate?);
    }
    Ok(bevy_crates)
}

//Takes  : crate.name
//Returns: crate.id,crate.name,crate.license,crate.latest_version,[bevy_versions_of_crate]
#[instrument]
#[rustfmt::skip]
#[allow(clippy::type_complexity)]
pub fn get_rev_dependency(
    db: &Connection, crate_name: &str, expected_crate_dep_name: &str,
) -> Result<
    Vec<
        Result<(String,String,String,String,Result<Vec<(String, String)>, cratesio_dbdump_csvtab::rusqlite::Error>,),cratesio_dbdump_csvtab::rusqlite::Error,>,
    >,
    cratesio_dbdump_csvtab::rusqlite::Error,
> {
    //Get just the bevy crate
    let expected_crate = get_crate_by_name(db, expected_crate_dep_name)?;
    //Get all bevy engine crates
    //let bevy_crate = get_bevy_crates(&db)?;
    let mut latest_crate = db.prepare(
        r#"
        SELECT crates.id, crates.name, versions.license, versions.num, versions.id 
        FROM versions LEFT JOIN crates 
        ON versions.crate_id = crates.id 
        WHERE crates.name = ?
        ORDER BY versions.num DESC
        LIMIT 1
        "#,
    )?;
    let row = latest_crate.query_map(
        // [vec!["bevy_prototype_lyon", "bevy_egui", "bevy_webgl2"]],
        [crate_name],
        |r| {
            Ok((
                r.get_unwrap::<_, String>(0),
                r.get_unwrap::<_, String>(1),
                r.get_unwrap::<_, String>(2),
                r.get_unwrap::<_, String>(3),
                get_bevy_versions_for_crate(
                    db,
                    &r.get_unwrap::<_, String>(0),
                    &r.get_unwrap::<_, String>(4),
                    &expected_crate,
                ),
            ))
        },
    )?;
    Ok(row.collect::<Vec<_>>())
}

#[instrument]
#[rustfmt::skip]
#[allow(clippy::type_complexity)]
pub fn crate_list_get_rev_dependency(
    db: &Connection, crate_names: Vec<&str>, expected_crate_dep_name: &str,
) -> Result<
    Vec<Vec<Result<(String,String,String,String,Result<Vec<(String, String)>, cratesio_dbdump_csvtab::rusqlite::Error>,),cratesio_dbdump_csvtab::rusqlite::Error,>,>,>,
    cratesio_dbdump_csvtab::rusqlite::Error,
> {
    let mut bevy_crates = Vec::new();
    for crate_name in crate_names.iter() {
        bevy_crates.push(get_rev_dependency(
            db,
            crate_name,
            expected_crate_dep_name,
        )?);
    }
    Ok(bevy_crates)
}

#[instrument]
fn get_bevy_crates(db: &Connection) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s =
        db.prepare_cached("SELECT id, name FROM crates WHERE homepage = ? AND repository = ?")?;
    let rows = s.query_and_then(
        [
            "https://bevyengine.org",
            "https://github.com/bevyengine/bevy",
        ],
        |r| -> Result<(String, String), rusqlite::Error> {
            Ok((r.get_unwrap::<_, String>(0), r.get_unwrap::<_, String>(1)))
        },
    )?;
    let mut bevy_crates = Vec::new();
    for bevy_crate in rows {
        bevy_crates.push(bevy_crate?);
    }
    Ok(bevy_crates)
}

#[instrument]
pub fn get_bevy_versions_for_crate(
    db: &Connection, cid: &str, vid: &str, bevy_crates: &[(String, String)],
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s = db.prepare_cached(
        r#"
        SELECT dependencies.req
        FROM dependencies LEFT JOIN versions
            ON dependencies.version_id = versions.id 
            WHERE versions.crate_id = ? AND versions.id = ? AND dependencies.crate_id = ?
    "#,
    )?;
    let mvbv: Vec<(String, String)> = bevy_crates
        .iter()
        .map(|(bid, name)| -> Result<(String, String), rusqlite::Error> {
            Ok((
                s.query_row([cid, vid, bid], |r| r.get::<_, String>(0))?,
                name.to_string(),
            ))
        })
        .filter_map(|f| f.ok())
        .collect();
    Ok(mvbv)
}

fn get_keywords(db: &Connection, crate_id: String) -> Result<Vec<String>, rusqlite::Error> {
    let mut s = db.prepare_cached(
        "SELECT keywords.keyword 
            FROM keywords 
            LEFT JOIN crates_keywords
                ON keywords.id = crates_keywords.keyword_id
            WHERE crates_keywords.crate_id = ?",
    )?;
    let x = s
        .query_map([crate_id], |r| r.get::<_, String>(0))?
        .collect();
    x
}

pub fn get_crate(db: &Connection, crate_name: &str) -> Result<Option<Crate>, rusqlite::Error> {
    //Version id, Version num
    let a = get_latest(db, crate_name.to_string())?;
    let aa = a.last();
    if aa.is_none() {
        return Err(Error::InvalidQuery);
    }
    let version_id = &aa.unwrap().0;

    let mut b = db.prepare_cached(
        "SELECT crates.name, dependencies.req, dependencies.kind
        FROM dependencies
        LEFT JOIN crates
            ON crates.id = dependencies.crate_id 
        WHERE dependencies.version_id = ?",
    )?;
    let b_row = b.query_map([version_id], |r| {
        Ok((
            r.get_unwrap::<_, String>(0),
            r.get_unwrap::<_, String>(1),
            r.get_unwrap::<_, String>(2),
        ))
    })?;

    let v3 = b_row
        .filter(|f| f.is_ok())
        .map(|f| f.unwrap())
        .collect::<Vec<(String, String, String)>>();

    let mut s = db.prepare_cached(
        "SELECT id, name, description, downloads, homepage, repository, updated_at FROM crates WHERE name = ?",
    )?;
    let row = s.query_map([crate_name], |r| {
        Ok((
            r.get_unwrap::<_, String>(0),
            r.get_unwrap::<_, String>(1),
            r.get_unwrap::<_, String>(2),
            r.get_unwrap::<_, String>(3),
            r.get_unwrap::<_, String>(4),
            r.get_unwrap::<_, String>(5),
            r.get_unwrap::<_, String>(6),
            get_keywords(db, r.get_unwrap::<_, String>(0)).unwrap_or_default(),
        ))
    })?;

    let c = row
        .filter(|f| f.is_ok())
        .map(|f| f.unwrap())
        .map(|f| Crate {
            crateid: f.1,
            disc: f.2,
            downloads: (f.3).parse().unwrap_or_default(),
            homepage_url: if f.4.is_empty() { None } else { Some(f.4) },
            repo_url: if f.5.is_empty() { None } else { Some(f.5) },
            last_update: f.6,
            tags: f.7,
            dependencies: v3.clone(),
            ..Default::default()
        });
    Ok(c.into_iter().last())
}

#[derive(Default, Debug)]
pub struct Crate {
    pub crateid: String,
    pub tags: Vec<String>,
    pub versions: Vec<String>,
    pub disc: String,
    pub downloads: u32,
    pub repo_url: Option<String>,
    pub homepage_url: Option<String>,
    pub last_update: String,
    pub dependencies: Vec<(String, String, String)>,
}

#[cfg(feature = "capture-spantrace")]
pub fn install_tracing() {
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

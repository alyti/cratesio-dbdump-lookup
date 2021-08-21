use rusqlite::{self, Connection, Error as SqliteError};
use std::num::ParseIntError;
use tracing::instrument;
use thiserror::Error;
use serde::Serialize;


#[derive(Error, Debug)]
pub enum Error {
    #[error("lookup query error")]
    LookupError(#[from] SqliteError),

    #[error("unknown crate {0} (no versions available)")]
    UnknownCrate(String)
}

pub trait CrateLookup {
    fn get_crate(&self, crate_name: &str) -> Result<Option<Crate>, Error>;
    fn get_keywords(&self, crate_id: &str) -> Result<Vec<String>, Error>;
}

impl CrateLookup for Connection {
    fn get_crate(&self, crate_name: &str) -> Result<Option<Crate>, Error> {
        //Version id, Version num
        let versions = get_versions(self, crate_name.to_string(), false)?;
        let version = versions.last();
        if version.is_none() {
            return Err(Error::UnknownCrate(crate_name.to_string()));
        }
        let version_id = &version.unwrap().0;

        let mut dep_version_q = self.prepare_cached(
            "SELECT crates.name, dependencies.req, dependencies.kind
            FROM dependencies
            LEFT JOIN crates
                ON crates.id = dependencies.crate_id 
            WHERE dependencies.version_id = ?",
        )?;
        let dep_version_result = dep_version_q.query_map([version_id], |r| {
            Ok((
                r.get_unwrap::<_, String>(0),
                r.get_unwrap::<_, String>(1),
                r.get_unwrap::<_, String>(2),
            ))
        })?;

        //Name, Version, Kind(Normal/Dev)
        let dependencies_n_v_k = dep_version_result
            .filter(|f| f.is_ok())
            .map(|f| f.unwrap())
            .collect::<Vec<(String, String, String)>>();

        let mut crate_info_q = self.prepare_cached(
            "SELECT id, name, description, downloads, homepage, repository, updated_at FROM crates WHERE name = ?",
        )?;
        let crate_info_result = crate_info_q.query_map([crate_name], |r| {
            Ok((
                r.get_unwrap::<_, String>(0),
                r.get_unwrap::<_, String>(1),
                r.get_unwrap::<_, String>(2),
                r.get_unwrap::<_, String>(3),
                r.get_unwrap::<_, String>(4),
                r.get_unwrap::<_, String>(5),
                r.get_unwrap::<_, String>(6),
                self.get_keywords(r.get_unwrap::<_, String>(0).as_str())
                    .unwrap_or_default(),
            ))
        })?;

        let dependencies = dependencies_n_v_k
            .iter()
            .map(|f| CrateDependency {
                crate_id: f.0.clone(),
                version: f.1.clone(),
                kind: DependencyKind::parse(f.2.clone()),
            })
            .collect::<Vec<CrateDependency>>();

        let mapped_versions  = versions
            .iter()
            .map(|f| f.1.clone())
            .collect::<Vec<String>>();

        let fetched_crate_list = crate_info_result
            .filter(|f| f.is_ok())
            .map(|f| f.unwrap())
            .map(|f| Crate {
                crate_id: f.1,
                description: f.2,
                downloads: (f.3).parse().unwrap_or_default(),
                homepage_url: if f.4.is_empty() { None } else { Some(f.4) },
                repo_url: if f.5.is_empty() { None } else { Some(f.5) },
                last_update: f.6,
                keywords: f.7,
                dependencies: dependencies.clone(),
                versions: mapped_versions.clone(),
            });
        Ok(fetched_crate_list.into_iter().last())
    }

    fn get_keywords(&self, crate_id: &str) -> Result<Vec<String>, Error> {
        let mut keywords_q = self.prepare_cached(
            "SELECT keywords.keyword 
                FROM keywords 
                LEFT JOIN crates_keywords
                    ON keywords.id = crates_keywords.keyword_id
                WHERE crates_keywords.crate_id = ?",
        )?;
        let keywords_result = keywords_q
            .query_map([crate_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<String>, SqliteError>>().map_err(|e| Error::LookupError(e));
        keywords_result
    }
}

//Takes  : crate_name
//Returns: version_id,version number
#[instrument]
pub fn get_versions(
    db: &Connection,
    crate_name: String,
    latest: bool,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut q = r#"
        SELECT versions.id, versions.num
        FROM versions 
        LEFT JOIN crates
            ON versions.crate_id = crates.id 
        WHERE crates.name = ?
    "#.to_string();

    if latest{
        q = q.to_string() +
        r#"
        ORDER BY versions.num DESC
        LIMIT 1
        "#
    }

    let mut stmt = db.prepare_cached(
        &q,
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
    db: &Connection,
    crate_name: String,
    d_type: DependencyType,
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let latest = get_versions(db, crate_name, true)?;
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
    db: &Connection,
    version_id: &str,
    d_type: DependencyType,
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
    db: &Connection,
    crate_name: &str,
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
        Result<(String,String,String,String,Result<Vec<(String, String)>, rusqlite::Error>,),rusqlite::Error,>,
    >,
    rusqlite::Error,
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
                get_versions_for_crate(
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
    Vec<Vec<Result<(String,String,String,String,Result<Vec<(String, String)>, rusqlite::Error>,),rusqlite::Error,>,>,>,
    rusqlite::Error,
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

/**
 Takes a crate_id (cid), versionid (vid) and a tuple of crates (tuple of crate_id and crate name)
 dependency_crates gets the name of the
*/
#[instrument]
pub fn get_versions_for_crate(
    db: &Connection,
    cid: &str,
    vid: &str,
    dependency_crates: &[(String, String)],
) -> Result<Vec<(String, String)>, rusqlite::Error> {
    let mut s = db.prepare_cached(
        r#"
        SELECT dependencies.req
        FROM dependencies LEFT JOIN versions
            ON dependencies.version_id = versions.id 
            WHERE versions.crate_id = ? AND versions.id = ? AND dependencies.crate_id = ?
    "#,
    )?;
    let mvbv: Vec<(String, String)> = dependency_crates
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

#[derive(Default, Debug, Serialize)]
pub struct Crate {
    pub crate_id: String,
    pub keywords: Vec<String>,
    pub versions: Vec<String>,
    pub description: String,
    pub downloads: u32,
    pub repo_url: Option<String>,
    pub homepage_url: Option<String>,
    pub last_update: String,
    pub dependencies: Vec<CrateDependency>,
}

#[derive(PartialEq, Debug, Clone, Serialize)]
pub enum DependencyKind {
    Normal,
    Dev,
    Unknown,
}

impl Default for DependencyKind {
    fn default() -> Self {
        Self::Unknown
    }
}

trait Parse {
    fn parse(kind_num: String) -> DependencyKind;
}

impl Parse for DependencyKind {
    fn parse(kind_num: String) -> DependencyKind {
        let int: Result<i32, ParseIntError> = kind_num.parse();
        match int {
            Ok(x) => match x {
                0 => DependencyKind::Normal,
                2 => DependencyKind::Dev,
                _ => DependencyKind::Unknown,
            },
            Err(_) => DependencyKind::Unknown,
        }
    }
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct CrateDependency {
    pub crate_id: String,
    pub version: String,
    pub kind: DependencyKind,
}

use color_eyre::eyre::Report;
use cratesio_dbdump_csvtab::CratesIODumpLoader;
use cratesio_dbdump_lookup::DependencyType;
use cratesio_dbdump_lookup::{
    crate_list_get_rev_dependency, get_bevy_plugins_naive, get_crate_by_name, get_versions,
    get_latest_dependencies, get_rev_dependency, CrateLookup,
};
use tracing::info;

fn main() -> Result<(), Report> {
    #[cfg(feature = "capture-spantrace")]
    install_tracing();

    color_eyre::install()?;

    // Load dump from a .tar.gz archive.
    let db = CratesIODumpLoader::default()
        //minimal() does not work for get_crate if you want keywords to be included
        .tables(&[
            "crates",
            "dependencies",
            "versions",
            "crates_keywords",
            "keywords",
        ])
        .preload(true)
        .update()?
        .open_db()?;

    info!("Get crate by name: {:?}", get_crate_by_name(&db, "bevy")?);
    info!(
        "Get latest version with bevy as crate: {:?}",
        get_versions(&db, "bevy".to_string(), true)?
    );
    info!(
        "Get all versions with bevy as crate: {:?}",
        get_versions(&db, "bevy".to_string(), false)?
    );
    info!(
        "Get all of the latest version's dependencies: {:?}",
        get_latest_dependencies(&db, "bevy".to_string(), DependencyType::All)?
    );
    info!(
        "Get all of the latest version's dev-dependencies: {:?}",
        get_latest_dependencies(&db, "bevy".to_string(), DependencyType::Dev)?
    );
    info!(
        "Get all of the latest version's normal dependencies: {:?}",
        get_latest_dependencies(&db, "bevy".to_string(), DependencyType::Normal)?
    );

    info!(
        "Get bevy plugins (based on suffix): {:?}",
        get_bevy_plugins_naive(&db)?
    );

    info!(
        "Get rev dependencies: {:?}",
        get_rev_dependency(&db, "bevy_egui", "bevy")?
    );

    info!(
        "Get rev dependencies 2: {:?}",
        get_rev_dependency(&db, "quote", "bevy")?
    );

    info!(
        "Get crate list of rev dependencies 3: {:?}",
        crate_list_get_rev_dependency(
            &db,
            vec!["bevy_retrograde_physics", "bevy_ninepatch"],
            "bevy"
        )
    );

    let c = db.get_crate("bevy_config_cam");
    info!("Get Crate: {:?}", c);

    Ok(())
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

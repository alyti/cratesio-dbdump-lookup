use color_eyre::Report;
use cratesio_dbdump_csvtab::CratesIODumpLoader;
use populate_crate_metadata::{
    crate_list_get_rev_dependency, get_bevy_plugins_naive, get_crate_by_name, get_latest,
    get_latest_dependencies, get_rev_dependency,
};
use populate_crate_metadata::{get_crate, install_tracing, DependencyType};
use tracing::info;

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
    info!(
        "Get Latest with bevy as crate: {:?}",
        get_latest(&db, "bevy".to_string())?
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
        "Get rev dependencies 3: {:?}",
        crate_list_get_rev_dependency(
            &db,
            vec!["bevy_retrograde_physics", "bevy_ninepatch"],
            "bevy"
        )
    );

    let c = get_crate(&db, "bevy_config_cam");
    println!("get_crate: {:?}", c);

    Ok(())
}

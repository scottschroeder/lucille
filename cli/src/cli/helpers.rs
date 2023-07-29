use anyhow::Context;
use app::app::LucilleApp;

use super::argparse;

#[deprecated(note = "use the app builder interface")]
pub async fn get_app(
    db_args: Option<&argparse::DatabaseConfig>,
    storage_args: Option<&argparse::StorageConfig>,
) -> anyhow::Result<LucilleApp> {
    app::app::LucilleBuilder::new_with_user_dirs()
        .context("could not create app builder")?
        .index_root(storage_args.and_then(|a| a.index_root()))
        .context("could not set index root")?
        .database_path(db_args.and_then(|a| a.database_path()))
        .context("could not set database")?
        .build()
        .await
        .context("could not build app config")
}

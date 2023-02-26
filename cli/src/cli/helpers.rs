use app::app::LucileApp;

use super::argparse;

#[deprecated(note = "use the app builder interface")]
pub async fn get_app(
    db_args: Option<&argparse::DatabaseConfig>,
    storage_args: Option<&argparse::StorageConfig>,
) -> anyhow::Result<LucileApp> {
    Ok(app::app::LucileBuilder::new()?
        .index_root(storage_args.and_then(|a| a.index_root()))?
        .database_path(db_args.and_then(|a| a.database_path()))?
        .build()
        .await?)
}

use app::app::LucileApp;

use super::argparse;

pub async fn get_app(
    db_args: Option<&argparse::DatabaseConfig>,
    storage_args: Option<&argparse::StorageConfig>,
) -> anyhow::Result<LucileApp> {
    Ok(LucileApp::create(
        db_args.and_then(|o| o.database_path.as_ref()),
        storage_args.and_then(|o| o.index_root.as_ref()),
    )
    .await?)
}

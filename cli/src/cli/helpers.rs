use app::app::LucileApp;
use database::Database;

// use crate::{service::search::SearchRequest, storage::Storage};

use super::argparse;

// pub fn get_search_request<'a>(args: &'a clap::ArgMatches) -> anyhow::Result<SearchRequest<'a>> {
//     Ok(SearchRequest {
//         query: args.value_of("query").unwrap(),
//         window: args
//             .value_of("search_window")
//             .map(|s| s.parse::<usize>())
//             .transpose()?,
//         max_responses: Some(5),
//     })
// }

// pub fn get_storage(args: &clap::ArgMatches) -> Storage {
//     let storage_path = args.value_of("storage").unwrap();
//     let index_name = args.value_of("index_name").unwrap_or("default");
//     let storage_path = std::path::Path::new(storage_path);
//     Storage::new(storage_path, index_name)
// }


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

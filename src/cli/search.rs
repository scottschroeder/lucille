use crate::service::search::{SearchClient, SearchService};
use anyhow::Result;

use crate::cli::helpers::{get_search_request, get_storage};

pub fn search(args: &clap::ArgMatches) -> Result<()> {
    let s = get_storage(args);

    let db = s.load()?;
    let index = s.index()?;
    let search_service = SearchService::new(db.id, index, &db.content);

    let search_request = get_search_request(args)?;

    let search_response = search_service.search(search_request)?;
    println!("{}", serde_json::to_string_pretty(&search_response)?);

    Ok(())
}

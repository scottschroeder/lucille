use crate::service::{
    search::{SearchClient, SearchService},
    transcode::{NamedFileOutput, TranscodeClient, TranscodeRequest, TranscoderService},
};
use anyhow::Result;

use crate::cli::helpers::{get_search_request, get_storage};

pub fn interactive(args: &clap::ArgMatches) -> Result<()> {
    let output = args.value_of("output_gif").unwrap();
    let s = get_storage(args);

    let db = s.load()?;
    let index = s.index()?;
    let search_service = SearchService::new(db.id, index, &db.content);
    let gif_output = NamedFileOutput(output.to_string());
    let transcode_service = TranscoderService::new(db.id, &db.content, &db.videos, &gif_output);

    let search_request = get_search_request(args)?;

    let search_response = search_service.search(search_request)?;
    let clip = crate::cli_select::ask_user_for_clip(&db.content, &search_response)?;
    let transcode_request = TranscodeRequest { clip };
    let transcode_response = transcode_service.transcode(transcode_request)?;

    println!("{:?}", transcode_response);

    Ok(())
}

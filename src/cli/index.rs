use crate::{
    service::{
        search::{SearchClient, SearchRequest, SearchService},
        transcode::{
            ClipIdentifier, NamedFileOutput, TranscodeClient, TranscodeRequest, TranscoderService,
        },
    },
    storage::Storage,
};
use anyhow::Result;

pub fn index(args: &clap::ArgMatches) -> Result<()> {
    let content_path = args.value_of("path").unwrap();
    let storage_path = args.value_of("storage").unwrap();
    let max_window = args.value_of("index_window").unwrap().parse::<usize>()?;
    let storage_path = std::path::Path::new(storage_path);

    std::fs::remove_dir_all(storage_path)?;

    let (content, videos) = crate::content::scan::scan_filesystem(content_path)?;
    let s = Storage::new(storage_path);
    let _index = s.build_index(content, videos, max_window)?;
    Ok(())
}

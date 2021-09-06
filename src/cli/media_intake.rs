use crate::{
    cli::helpers::{get_search_request, get_storage},
    content::scan::scan_filesystem,
    service::{
        search::{SearchClient, SearchRequest, SearchService},
        transcode::{
            ClipIdentifier, NamedFileOutput, TranscodeClient, TranscodeRequest, TranscoderService,
        },
    },
    storage::Storage,
};
use anyhow::Result;
use std::time::Duration;

pub fn scan_titles(args: &clap::ArgMatches) -> Result<()> {
    let p = std::path::Path::new(args.value_of("path").unwrap());
    // ffmpeg::output_csv_reader(p);
    // return Ok(());
    log::debug!("scan titles: {:?}", p);
    let (content, fs_content) = scan_filesystem(p)?;
    let (media, files) = crate::details::process::intake_media(content, fs_content);
    log::debug!("{:#?}", media);
    log::debug!("{:#?}", files);

    let fs = crate::details::storage::FileStorage::new("storage_backend")?;
    let splitter = crate::details::transform::FFMpegShellSplitter::new(Duration::from_secs(30));
    let x = crate::details::process::split_media(&fs, &splitter, files)?;
    log::debug!("{:#?}", x);

    Ok(())
}

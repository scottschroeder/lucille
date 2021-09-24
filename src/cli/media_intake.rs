use crate::{
    cli::helpers::get_storage,
    content::{
        process::content_metadata,
        scan::{process_media, scan_media_paths},
        split::MediaSplitter,
        storage::Storage as _,
        SegmentedVideo,
    },
};
use anyhow::{Context, Result};
use std::time::Duration;

pub fn scan_titles(args: &clap::ArgMatches) -> Result<()> {
    let p = std::path::Path::new(args.value_of("path").unwrap());
    log::debug!("scan titles: {:?}", p);
    let contents = scan_media_paths(p)?;
    let media = process_media(contents.as_slice());
    let (title, files, content) = content_metadata(media);
    let s = get_storage(args);
    s.prepare().context("could not prepare storage")?;
    for c in &content {
        s.write_content(c.media_hash, c)
            .context("could not write content")?;
    }
    s.write_content_db(title, files)?;
    Ok(())
}

pub fn index(args: &clap::ArgMatches) -> Result<()> {
    let s = get_storage(args);
    let max_window = args.value_of("index_window").unwrap().parse::<usize>()?;
    let _index = s.build_index(max_window)?;
    Ok(())
}

pub fn prepare(args: &clap::ArgMatches) -> Result<()> {
    let storage = get_storage(args);
    let fs = crate::content::storage::FileStorage::new(storage.storage_path())?;
    let splitter = crate::content::split::FFMpegShellSplitter::new(Duration::from_secs(30));
    let db = storage.load_content_db().context("could not load db")?;

    match &db.media_listing {
        crate::storage::media_listing::MediaListing::MediaIds(_) => {
            anyhow::bail!("unable to re-encode video without original media")
        }
        crate::storage::media_listing::MediaListing::MediaPaths(original_files) => {
            for (k, v) in original_files {
                if storage.load_media_map(k).is_ok() {
                    continue;
                }
                let m = splitter.clone();
                let segments = m.chop_into_segments(&v)?;
                let mut video = Vec::new();
                for s in segments {
                    fs.insert_file(s.segment.0, s.path.as_path())?;
                    video.push((s.segment, s.position))
                }
                storage
                    .save_media_map(k, &SegmentedVideo { inner: video })
                    .context("could not save media map")?;
            }
        }
    }

    Ok(())
}

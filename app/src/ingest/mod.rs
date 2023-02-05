use lucile_core::metadata::{MediaHash, MediaMetadata};
use rayon::prelude::*;

mod extract;
mod insert;
mod metadata;
mod scan;

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub enum ScannedSubtitles {
    NotFound,
    Error(subrip::Error),
    Subtitles(Vec<subrip::Subtitle>),
}

#[derive(Debug)]
pub(crate) struct ScannedData {
    pub path: std::path::PathBuf,
    pub subs: ScannedSubtitles,
    pub hash: MediaHash,
}

#[derive(Debug)]
pub struct ScannedMedia {
    pub path: std::path::PathBuf,
    pub subs: ScannedSubtitles,
    pub hash: MediaHash,
    pub metadata: MediaMetadata,
}

pub use insert::add_content_to_corpus;

pub use scan::scan_media_paths;

/// batch process a list of media paths
pub fn process_media_in_parallel(paths: &[std::path::PathBuf]) -> Vec<ScannedMedia> {
    paths
        .into_par_iter()
        .map(|p| (p, process_single_media3(p.as_path())))
        .filter_map(|(p, r)| match r {
            Ok(m) => Some((p.to_owned(), m)),
            Err(e) => {
                log::warn!("unable to use {:?}: {}", p, e);
                None
            }
        })
        .map(|(_p, r)| r)
        .collect()
}

fn process_single_media3(media_path: &std::path::Path) -> Result<ScannedMedia, ScanError> {
    extract::read_media_from_path(media_path).map(|data| data.extract_metadata())
}

/*
 * I'm trying to take `app/src/scan.rs` and break it apart.
 *
 * ingest::scan should trawl the FS for media paths
 * ingest::extract should do anything that immediately requires the FS
 *  - in practice, that is getting the media hash, and extracting subs
 * ingest::<what> should do the logical application update to the db and
 *  we should test the bejesus out of it
 */

use std::{
    io::Read,
    path::{self, PathBuf},
};

use lucile_core::{
    hash::Sha2Hash,
    metadata::{EpisodeMetadata, MediaHash, MediaMetadata},
};
use rayon::prelude::*;
use sha2::{Digest, Sha256};

const MEDIA_FILES: &[&str] = &["mkv"];

pub enum ScannedSubtitles {
    NotFound,
    Error(subrip::Error),
    Subtitles(Vec<subrip::Subtitle>),
}

impl std::fmt::Debug for ScannedSubtitles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "NotFound"),
            Self::Error(arg0) => f.debug_tuple("Error").field(arg0).finish(),
            Self::Subtitles(arg0) => f.debug_tuple("Subtitles").field(&arg0.len()).finish(),
        }
    }
}

#[derive(Debug)]
pub struct ScannedMedia {
    pub path: PathBuf,
    pub subs: ScannedSubtitles,
    pub hash: MediaHash,
    pub title: String,
    pub metadata: MediaMetadata,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// is a path media we care about?
fn is_media(p: &path::Path) -> bool {
    let oext = p.extension();
    oext.and_then(|ext| ext.to_str())
        .map(|ext| MEDIA_FILES.contains(&ext))
        .unwrap_or(false)
}

fn read_path_to_string<P: AsRef<path::Path>>(tpath: P) -> Result<String, ScanError> {
    let tpath = tpath.as_ref();
    let mut f = std::fs::File::open(tpath)?;
    let mut v = Vec::new();
    f.read_to_end(&mut v)?;

    Ok(match String::from_utf8(v) {
        Ok(s) => s,
        Err(e) => {
            let v = e.into_bytes();
            // SRT files are WINDOWS_1252 by default, but there is no requirement, so who knows
            let (text, encoding, replacements) = encoding_rs::WINDOWS_1252.decode(v.as_slice());
            if replacements {
                log::warn!(
                    "could not decode {:?} accurately with {}",
                    tpath,
                    encoding.name()
                );
            }
            text.to_string()
        }
    })
}

/// Get the list of paths
fn scan_media_paths<P: AsRef<path::Path>>(root: P) -> std::io::Result<Vec<path::PathBuf>> {
    let root = root.as_ref();
    let mut content = Vec::new();
    for dir in walkdir::WalkDir::new(root)
        .into_iter()
        .filter(|de| de.as_ref().map(|de| is_media(de.path())).unwrap_or(true))
    {
        let dir = dir?;
        log::trace!("scanned: {:?}", dir.path());
        content.push(dir.path().to_owned());
    }

    Ok(content)
}

/// Get all content under a directory
pub fn scan_content<P: AsRef<path::Path>>(root: P) -> Result<Vec<ScannedMedia>, ScanError> {
    let contents = scan_media_paths(root)?;
    let res = process_media_in_parallel(&contents);
    Ok(res)
}

/// find/extract subtitles for a given piece of media
fn extract_subtitles(media_path: &path::Path) -> Result<ScannedSubtitles, ScanError> {
    let srt_path = media_path.with_extension("srt");
    if !srt_path.exists() {
        return Ok(ScannedSubtitles::NotFound);
    }
    let srt_contents = read_path_to_string(srt_path.as_path())?;
    Ok(match subrip::parse(&srt_contents) {
        Ok(s) => ScannedSubtitles::Subtitles(s),
        Err(e) => ScannedSubtitles::Error(e),
    })
}
/// Get the sha2 hash for a media path
fn hash_video(p: &path::Path) -> Result<MediaHash, ScanError> {
    let mut file = std::fs::File::open(p)?;
    let mut hasher = Sha256::new();

    // TODO FIXME DO NOT MERGE temp hack to speed up test cycle
    // let mut file = std::io::Cursor::new(p.as_os_str().as_bytes());

    std::io::copy(&mut file, &mut hasher)?;
    Ok(MediaHash::new(Sha2Hash::from(hasher.finalize())))
}

// /// get details about a single piece of media
// fn process_single_media(media_path: &path::Path) -> Result<ContentFileDetails> {
//     let subtitles = extract_subtitles(media_path)?;
//     let media_hash = hash_video(media_path)?;
//     let title = title(media_path)?;
//     let episode = ContentFileDetails {
//         title,
//         subtitles,
//         media_hash,
//     };
//     log::trace!("{:?}", episode);
//     Ok(episode)
// }

fn process_single_media2(media_path: &path::Path) -> Result<ScannedMedia, ScanError> {
    let subtitles = extract_subtitles(media_path)?;
    let media_hash = hash_video(media_path)?;
    let title = title(media_path).unwrap_or_else(|| media_path.to_string_lossy().to_string());
    let metadata = extract_metadata(&title);

    Ok(ScannedMedia {
        path: media_path.to_path_buf(),
        subs: subtitles,
        hash: media_hash,
        title,
        metadata,
    })
}

/// batch process a list of media paths
fn process_media_in_parallel(paths: &[path::PathBuf]) -> Vec<ScannedMedia> {
    paths
        .into_par_iter()
        .map(|p| (p, process_single_media2(p.as_path())))
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

/// get the title for a media path
/// TODO: this is a really primitive implementation
fn title(p: &path::Path) -> Option<String> {
    let file_name = p.file_name()?;
    Some(file_name.to_string_lossy().to_string())
}

/// Get rich-ish metadata from a media's title
fn extract_metadata(title: &str) -> MediaMetadata {
    match torrent_name_parser::Metadata::from(title) {
        Ok(m) => {
            if let Some((s, e)) = m.season().zip(m.episode()) {
                MediaMetadata::Episode(EpisodeMetadata {
                    season: s as u32,
                    episode: e as u32,
                    title: hacky_extract_episode_name(title),
                })
            } else {
                MediaMetadata::Unknown(m.title().to_string())
            }
        }
        Err(e) => {
            log::warn!("could not parse metadata from `{:?}`: {}", title, e);
            MediaMetadata::Unknown(title.to_string())
        }
    }
}

// TODO: this only happens to work because of my data
fn hacky_extract_episode_name(filename: &str) -> String {
    let segments = filename.split('.').collect::<Vec<_>>();
    let r = segments[2..segments.len() - 1].join(".");
    if r.is_empty() {
        filename.to_string()
    } else {
        r
    }
}
// /// process an entire collection for metadata
// fn content_metadata(media: Vec<(path::PathBuf, ContentFileDetails)>) -> ContentScanResults {
//     let mut content_name_guesser = HashMap::new();
//     let mut content = Vec::new();
//     let mut media_file_map = HashMap::new();
//     for (path, e) in media {
//         let (metadata, name_guess) = extract_metadata(e.title.as_str());
//         if let Some(name) = name_guess {
//             *content_name_guesser.entry(name).or_insert(0) += 1;
//         }
//         let content_data = ContentData {
//             subtitle: e.subtitles,
//             media_hash: e.media_hash,
//             metadata,
//         };
//         media_file_map.insert(e.media_hash, path);
//         content.push(content_data);
//     }
//     let content_name = content_name_guesser
//         .into_iter()
//         .max_by_key(|(_, v)| *v)
//         .map(|(s, _)| s);

//     ContentScanResults {
//         suggested_name: content_name,
//         files: media_file_map,
//         content,
//     }
// }

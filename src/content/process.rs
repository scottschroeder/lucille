use super::{
    index::{
        ContentData, EpisodeMetadata, MediaHash, MediaId, MediaMetadata,
    },
};
use crate::content::{Episode, VideoFile};

use std::{collections::HashMap, path};

fn extract_metadata(title: &str) -> (MediaMetadata, Option<String>) {
    let mut name_guess = None;
    let metadata = match torrent_name_parser::Metadata::from(title) {
        Ok(m) => {
            name_guess = Some(m.title().to_string());
            if let Some((s, e)) = m.season().zip(m.episode()) {
                MediaMetadata::Episode(EpisodeMetadata {
                    season: s as u32,
                    episode: e as u32,
                    title: title.to_string(),
                })
            } else {
                MediaMetadata::Unknown(m.title().to_string())
            }
        }
        Err(e) => {
            log::warn!("could not parse metadata from `{:?}`: {}", title, e);
            MediaMetadata::Unknown(title.to_string())
        }
    };
    (metadata, name_guess)
}

#[derive(Debug)]
pub struct MediaFileMap {
    inner: HashMap<MediaId, VideoFile>,
}

pub fn content_metadata(
    media: Vec<(path::PathBuf, Episode)>,
) -> (String, HashMap<MediaHash, path::PathBuf>, Vec<ContentData>) {
    let mut content_name_guesser = HashMap::new();
    let mut content = Vec::new();
    let mut media_file_map = HashMap::new();
    for (path, e) in media {
        let (metadata, name_guess) = extract_metadata(e.title.as_str());
        if let Some(name) = name_guess {
            *content_name_guesser.entry(name).or_insert(0) += 1;
        }
        let content_data = ContentData {
            subtitle: e.subtitles,
            media_hash: e.media_hash,
            metadata,
        };
        media_file_map.insert(e.media_hash, path);
        content.push(content_data);
    }
    let content_name = content_name_guesser
        .into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(s, _)| s)
        .unwrap_or_else(|| "Unknown".to_string());
    (content_name, media_file_map, content)
}

use anyhow::Result;

use crate::content::{Content, FileSystemContent, VideoFile};
use std::collections::HashMap;

use super::{
    index::{
        ContentData, ContentMetadata, ContentSegments, EpisodeMetadata, MediaId, MediaMetadata,
        RawMediaResults, SegmentedVideo, Uuid,
    },
    storage::Storage,
    transform::MediaSplitter,
};

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

pub fn intake_media(content: Content, files: FileSystemContent) -> (RawMediaResults, MediaFileMap) {
    let content_id = Uuid::new();

    let mut content_map = HashMap::new();
    let mut content_name_guesser = HashMap::new();
    let mut media_file_map = HashMap::new();
    for (episode, video_file) in content.episodes.into_iter().zip(files.videos.into_iter()) {
        let media_id = MediaId(Uuid::new());
        let (metadata, name_guess) = extract_metadata(episode.title.as_str());
        if let Some(name) = name_guess {
            *content_name_guesser.entry(name).or_insert(0) += 1;
        }
        let content_data = ContentData {
            subtitle: episode.subtitles,
            metadata,
        };

        media_file_map.insert(media_id, video_file);
        content_map.insert(media_id, content_data);
    }
    log::trace!("guesser: {:#?}", content_name_guesser);
    let content_name = content_name_guesser
        .into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(s, _)| s)
        .unwrap_or_else(|| "Unknown".to_string());
    (
        RawMediaResults {
            content_id,
            media: ContentMetadata { inner: content_map },
            content_name,
        },
        MediaFileMap {
            inner: media_file_map,
        },
    )
}

pub fn split_media<S: Storage, M: MediaSplitter>(
    storage: &S,
    media_splitter: &M,
    files: MediaFileMap,
) -> Result<ContentSegments> {
    let mut segment_map = HashMap::new();
    for (k, v) in files.inner {
        let m = media_splitter.clone();
        let segments = m.chop_into_segments(&v.0)?;
        let mut video = Vec::new();
        for s in segments {
            storage.insert_file(s.segment.0, s.path.as_path())?;
            video.push((s.segment, s.position))
        }
        segment_map.insert(k, SegmentedVideo { inner: video });
    }
    Ok(ContentSegments { inner: segment_map })
}

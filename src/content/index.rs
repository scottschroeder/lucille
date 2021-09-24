use crate::{content::video_range, hash::Sha2Hash, srt::Subtitles};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Uuid(uuid::Uuid);

impl Uuid {
    pub fn new() -> Uuid {
        Uuid(uuid::Uuid::new_v4())
    }
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaHash(Sha2Hash);

impl MediaHash {
    pub fn new(hash: Sha2Hash) -> MediaHash {
        MediaHash(hash)
    }
}

impl fmt::Display for MediaHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VideoSegmentId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct MediaTimestamp(pub Duration);

#[derive(Debug)]
pub struct ContentMetadata {
    pub inner: HashMap<MediaId, ContentData>,
}

#[derive(Debug)]
pub struct RawMediaResults {
    pub content_id: Uuid,
    pub content_name: String,
    pub media: ContentMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MediaMetadata {
    Episode(EpisodeMetadata),
    Unknown(String),
}

impl MediaMetadata {
    pub fn title(&self) -> String {
        match self {
            MediaMetadata::Episode(e) => e.title.clone(),
            MediaMetadata::Unknown(s) => s.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EpisodeMetadata {
    pub season: u32,
    pub episode: u32,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentData {
    pub metadata: MediaMetadata,
    pub media_hash: MediaHash,
    pub subtitle: Subtitles,
}

#[derive(Debug)]
pub struct ContentSegments {
    pub inner: HashMap<MediaHash, SegmentedVideo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SegmentedVideo {
    pub inner: Vec<(VideoSegmentId, MediaTimestamp)>,
}

impl SegmentedVideo {
    pub fn get_range(&self, start: MediaTimestamp, end: MediaTimestamp) -> Vec<VideoSegmentId> {
        video_range::extract_range(start, end, self.inner.as_slice())
            .cloned()
            .collect()
    }
}

use crate::{
    hash::Sha2Hash,
    srt::{Subtitle, Subtitles},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Pointer},
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

/*
    A bunch of raw episodes + sub files
    - a content id
    - list of media ids
    - subtitle <-> media id match
    - title <-> media id match
    - media id w start/end timers


    Split the raw episodes into segments
    - list of media ids
    - assoc segment id to media id
    - media id w start/end timers

    Encrypt segments
    - list of encrypted ids
    - name of key
    - assoc encrypted segment ids to segment media id
*/

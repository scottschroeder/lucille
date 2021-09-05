use std::{collections::HashMap, fmt, time::Duration};

use crate::srt::Subtitle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoSegmentId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
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

#[derive(Debug)]
pub enum MediaMetadata {
    Episode(EpisodeMetadata),
    Unknown(String),
}

#[derive(Debug)]
pub struct EpisodeMetadata {
    pub season: u32,
    pub episode: u32,
    pub title: String,
}

pub struct ContentData {
    pub metadata: MediaMetadata,
    pub subtitle: Vec<Subtitle>,
}

impl fmt::Debug for ContentData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContentData")
            .field("metadata", &self.metadata)
            .field("subtitle", &self.subtitle.len())
            .finish()
    }
}

#[derive(Debug)]
pub struct ContentSegments {
    pub inner: HashMap<MediaId, SegmentedVideo>,
}

#[derive(Debug)]
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

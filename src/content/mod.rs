use self::identifiers::MediaId;
pub use self::{
    identifiers::{MediaHash, Uuid},
    metadata::ContentData,
    split::SegmentedVideo,
};
use crate::srt::Subtitles;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap, time::Duration};

mod encrypted;
mod identifiers;
mod metadata;
pub mod process;
pub mod scan;
pub mod split;
pub mod storage;
mod video_range;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct MediaTimestamp(pub Duration);

// TODO what is this?
#[derive(Debug)]
pub struct ContentMetadata {
    pub inner: HashMap<MediaId, ContentData>,
}

// TODO what is this?
#[derive(Debug)]
pub struct RawMediaResults {
    pub content_id: Uuid,
    pub content_name: String,
    pub media: ContentMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Content {
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub title: String,
    pub subtitles: Subtitles,
    pub media_hash: MediaHash,
}

pub trait VideoSource {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str>;
    fn ffmpeg_type(&self) -> Option<String> {
        None
    }
}

impl<'s> VideoSource for &'s str {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str> {
        Cow::from(*self)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileSystemContent {
    pub videos: HashMap<MediaHash, VideoFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFile(pub String);

impl VideoSource for VideoFile {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str> {
        self.0.as_str().into()
    }
}

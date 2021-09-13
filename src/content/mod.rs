use crate::{details::MediaHash, srt::Subtitles};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap};

pub mod scan;

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

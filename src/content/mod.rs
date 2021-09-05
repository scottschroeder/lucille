use crate::srt::Subtitle;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, ffi::OsStr, fmt};

pub mod scan;

#[derive(Debug, Serialize, Deserialize)]
pub struct Content {
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Episode {
    pub title: String,
    pub subtitles: Vec<Subtitle>,
}

impl fmt::Debug for Episode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Episode")
            .field("title", &self.title)
            .field("subtitles", &self.subtitles.len())
            .finish()
    }
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
    pub videos: Vec<VideoFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFile(pub String);

impl VideoSource for VideoFile {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str> {
        self.0.as_str().into()
    }
}

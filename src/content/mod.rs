use crate::srt::Subtitle;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub mod scan;

#[derive(Serialize, Deserialize)]
pub struct Content {
    pub episodes: Vec<Episode>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Episode {
    pub title: String,
    pub subtitles: Vec<Subtitle>,
}

pub trait VideoSource {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str>;
    fn ffmpeg_type(&self) -> Option<String> {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct FileSystemContent {
    pub videos: Vec<VideoFile>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VideoFile(String);

impl VideoSource for VideoFile {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str> {
        self.0.as_str().into()
    }
}

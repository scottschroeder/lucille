use crate::{content::identifiers::MediaHash, srt::Subtitles};
use serde::{Deserialize, Serialize};

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

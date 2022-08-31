use std::fmt;

use serde::{Deserialize, Serialize};

use crate::hash::Sha2Hash;

#[derive(Debug, Serialize, Deserialize)]
pub struct EpisodeMetadata {
    pub season: u32,
    pub episode: u32,
    pub title: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaHash(Sha2Hash);

impl MediaHash {
    pub fn new(hash: Sha2Hash) -> MediaHash {
        MediaHash(hash)
    }
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl fmt::Display for MediaHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

use crate::hash::Sha2Hash;
use serde::{Deserialize, Serialize};
use std::fmt::{self};

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

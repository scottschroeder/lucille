use self::{metadata::MediaHash, identifiers::CorpusId};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
pub use subrip::Subtitle;

pub mod hash;
pub mod identifiers;
pub mod metadata;

pub mod export {

    use serde::{Deserialize, Serialize};

    use crate::ContentData;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CorpusExport {
        pub title: String,
        pub content: Vec<ContentData>,
    }
}

pub mod uuid {
    use std::{fmt::Display, str::FromStr};

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct Uuid(uuid::Uuid);

    impl Uuid {
        pub fn generate() -> Self {
            Uuid(uuid::Uuid::new_v4())
        }
    }

    impl FromStr for Uuid {
        type Err = uuid::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Uuid(uuid::Uuid::from_str(s)?))
        }
    }

    impl Display for Uuid {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }
}

pub mod clean_sub;

pub mod storage {
    use std::path::PathBuf;

    #[derive(Debug)]
    pub struct Storage {
        pub index_root: PathBuf,
    }
}

#[derive(Serialize, Deserialize)]
pub struct ContentData {
    pub metadata: metadata::MediaMetadata,
    pub hash: MediaHash,
    pub srt_id: u64,
    pub subtitle: Vec<Subtitle>,
}

impl Debug for ContentData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentData")
            .field("metadata", &self.metadata)
            .field("hash", &self.hash)
            .field("srt_id", &self.srt_id)
            .field("subtitle", &self.subtitle.len())
            .finish()
    }
}

#[derive(Debug)]
pub struct Corpus {
    pub id: Option<CorpusId>,
    pub title: String,
}

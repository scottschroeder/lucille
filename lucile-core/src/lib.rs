use std::fmt::Debug;

use serde::{Deserialize, Serialize};
pub use subrip::Subtitle;

use self::{identifiers::CorpusId, metadata::MediaHash};

pub mod hash;
pub mod identifiers;
pub mod metadata;

pub mod export {

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::{
        identifiers::{ChapterId, CorpusId, StorageId},
        metadata::{MediaHash, MediaMetadata},
        ContentData,
    };

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CorpusExport {
        pub title: String,
        pub content: Vec<MediaExport>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct MediaExport {
        pub views: ViewOptions,
        pub data: ContentData,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct ViewOptions {
        pub views: Vec<String>,
    }

    #[derive(Debug)]
    pub struct MediaStorage {
        pub id: StorageId,
        pub path: PathBuf,
        pub hash: MediaHash,
        pub exists_locally: Option<bool>,
        pub verified: bool,
    }

    #[derive(Debug)]
    pub struct ChapterExport {
        pub id: ChapterId,
        pub corpus_id: CorpusId,
        pub metadata: MediaMetadata,
        pub hash: MediaHash,
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

pub mod file_explorer {
    use std::path::PathBuf;

    use crate::{
        identifiers::ChapterId,
        media_segment::{MediaSegment, MediaView},
        metadata::{MediaHash, MediaMetadata},
        Corpus,
    };

    #[derive(Debug)]
    pub struct HashExploreResults {
        pub path: Option<PathBuf>,
        pub chapter: Option<ChapterExploreResults>,
        pub segment: Option<(MediaView, MediaSegment)>,
    }

    #[derive(Debug)]
    pub struct ChapterExploreResults {
        pub id: ChapterId,
        pub corpus: Corpus,
        pub metadata: MediaMetadata,
        pub hash: MediaHash,
    }
}

pub mod media_segment {
    use std::time::Duration;

    use crate::{
        identifiers::{ChapterId, MediaSegmentId, MediaViewId},
        metadata::MediaHash,
    };

    #[derive(Debug, PartialEq)]
    pub struct MediaView {
        pub id: MediaViewId,
        pub chapter_id: ChapterId,
        pub name: String,
    }

    pub struct EncryptionKey {
        key: String,
    }

    impl EncryptionKey {
        pub fn new<S: Into<String>>(key: S) -> EncryptionKey {
            EncryptionKey { key: key.into() }
        }
    }

    impl std::fmt::Debug for EncryptionKey {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("EncryptionKey")
                .field("key", &"<REDACTED>")
                .finish()
        }
    }

    #[derive(Debug)]
    pub struct MediaSegment {
        pub id: MediaSegmentId,
        pub media_view_id: MediaViewId,
        pub hash: MediaHash,
        pub start: Duration,
        pub key: Option<EncryptionKey>,
    }
}

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
    pub local_id: u64,
    pub global_id: uuid::Uuid,
    pub subtitle: Vec<Subtitle>,
}

impl Debug for ContentData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentData")
            .field("metadata", &self.metadata)
            .field("hash", &self.hash)
            .field("local_id", &self.local_id)
            .field("global_id", &self.global_id)
            .field("subtitle", &self.subtitle.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Corpus {
    pub id: Option<CorpusId>,
    pub title: String,
}

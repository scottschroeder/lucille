use std::{fmt::Debug, num::NonZeroI64};

pub use subrip::Subtitle;

pub mod hash;
pub mod metadata;

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

pub struct ContentData {
    pub metadata: metadata::MediaMetadata,
    pub srt_id: u64,
    pub subtitle: Vec<Subtitle>,
}

impl Debug for ContentData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentData")
            .field("metadata", &self.metadata)
            .field("subtitle", &self.subtitle.len())
            .finish()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
struct DbId(NonZeroI64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct CorpusId(DbId);

impl CorpusId {
    pub fn new(id: i64) -> CorpusId {
        CorpusId(DbId(
            NonZeroI64::new(id).expect("database id can not be zero"),
        ))
    }
    pub fn get(&self) -> i64 {
        self.0 .0.get()
    }
}

impl std::fmt::Display for CorpusId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ChapterId(DbId);

impl ChapterId {
    pub fn new(id: i64) -> ChapterId {
        ChapterId(DbId(
            NonZeroI64::new(id).expect("database id can not be zero"),
        ))
    }
    pub fn get(&self) -> i64 {
        self.0 .0.get()
    }
}

impl std::fmt::Display for ChapterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MediaViewId(DbId);

impl MediaViewId {
    pub fn new(id: i64) -> MediaViewId {
        MediaViewId(DbId(
            NonZeroI64::new(id).expect("database id can not be zero"),
        ))
    }
    pub fn get(&self) -> i64 {
        self.0 .0.get()
    }
}

impl std::fmt::Display for MediaViewId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

#[derive(Debug)]
pub struct Corpus {
    pub id: Option<CorpusId>,
    pub title: String,
}

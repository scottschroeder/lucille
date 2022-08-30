use std::num::NonZeroI64;
pub use subrip::Subtitle;

pub mod hash;
pub mod metadata;

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

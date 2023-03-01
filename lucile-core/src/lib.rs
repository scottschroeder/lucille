#![allow(clippy::uninlined_format_args)]
use std::fmt::Debug;

pub use metadata::MediaHash;
use serde::{Deserialize, Serialize};
pub use subrip::Subtitle;

use self::identifiers::CorpusId;

pub mod encryption_config;
pub mod hash;
pub mod identifiers;
pub mod metadata;

pub mod base64 {
    use std::fmt;

    use base64::Engine as _;
    use serde::de::DeserializeOwned;

    pub const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

    #[derive(Debug, thiserror::Error)]
    pub enum Base64Error {
        #[error(transparent)]
        Base64(#[from] base64::DecodeError),
        #[error(transparent)]
        SerdeJson(#[from] serde_json::Error),
    }

    pub struct B64Bytes<'a>(pub &'a [u8]);

    impl<'a> From<&'a str> for B64Bytes<'a> {
        fn from(value: &'a str) -> Self {
            Self(value.as_bytes())
        }
    }

    impl<'a> From<&'a [u8]> for B64Bytes<'a> {
        fn from(value: &'a [u8]) -> Self {
            Self(value)
        }
    }

    pub fn encode_string(bytes: impl AsRef<[u8]>) -> String {
        B64Bytes(bytes.as_ref()).to_string()
    }

    pub fn decode(encoded: &str) -> Result<Vec<u8>, Base64Error> {
        Ok(crate::base64::B64.decode(encoded)?)
    }

    pub fn deserialize_json<T: DeserializeOwned>(encoded: &str) -> Result<T, Base64Error> {
        let data = crate::base64::B64.decode(encoded)?;
        Ok(serde_json::from_slice(&data)?)
    }

    impl<'a> fmt::Display for B64Bytes<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let b64_wrapper = base64::display::Base64Display::new(
                self.0,
                &base64::engine::general_purpose::STANDARD,
            );

            write!(f, "{}", b64_wrapper)
        }
    }

    impl<'a> fmt::Debug for B64Bytes<'a> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self)
        }
    }

    pub mod serde_base64 {
        use base64::Engine as _;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        use super::B64;

        pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
            let base64 = B64.encode(v);
            String::serialize(&base64, s)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
            let base64 = String::deserialize(d)?;
            B64.decode(base64.as_bytes())
                .map_err(serde::de::Error::custom)
        }
    }
}

pub mod export {

    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::{
        identifiers::{ChapterId, CorpusId, StorageId},
        metadata::{MediaHash, MediaMetadata},
        ContentData,
    };

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CorpusExport {
        pub title: String,
        pub content: Vec<MediaExport>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MediaExport {
        pub views: ViewOptions,
        pub data: ContentData,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ViewOptions {
        pub views: Vec<String>,
    }

    #[derive(Debug, PartialEq)]
    pub struct MediaStorage {
        pub id: StorageId,
        pub path: PathBuf,
        pub hash: MediaHash,
        pub exists_locally: Option<bool>,
        pub verified: bool,
    }

    #[derive(Debug, Clone)]
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
        encryption_config::KeyData,
        identifiers::{ChapterId, MediaSegmentId, MediaViewId},
        metadata::MediaHash,
    };

    #[derive(Debug, PartialEq)]
    pub struct MediaView {
        pub id: MediaViewId,
        pub chapter_id: ChapterId,
        pub name: String,
    }

    #[derive(Clone, PartialEq)]
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

    #[derive(Debug, PartialEq)]
    pub struct MediaSegment {
        pub id: MediaSegmentId,
        pub media_view_id: MediaViewId,
        pub hash: MediaHash,
        pub start: Duration,
        pub key: Option<KeyData>,
    }
}

pub mod storage {
    use std::path::PathBuf;

    #[derive(Debug)]
    pub struct Storage {
        pub index_root: PathBuf,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentData {
    pub metadata: metadata::MediaMetadata,
    pub hash: MediaHash,
    pub subtitle: LucileSub,
}

#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub struct LucileSub {
    /// For use with local search index
    pub id: i64,
    /// A globally unique Id transferrable between instances
    pub uuid: uuid::Uuid,
    /// The actual subtitle data
    pub subs: Vec<Subtitle>,
}
impl Debug for LucileSub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LucileSub")
            .field("id", &self.uuid)
            .field("uuid", &self.uuid)
            .field("subtitle", &self.subs.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Corpus {
    pub id: Option<CorpusId>,
    pub title: String,
}

pub mod test_util {
    use std::time::Duration;

    use super::*;

    pub fn generate_subtitle(lines: &[&str]) -> Vec<Subtitle> {
        const INTERVAL: Duration = Duration::from_millis(1500);
        let mut subs = Vec::with_capacity(lines.len());
        let mut t = Duration::default();

        for (idx, txt) in lines.iter().enumerate() {
            let t2 = t.saturating_add(INTERVAL);
            subs.push(Subtitle {
                idx: idx as u32,
                start: t,
                end: t2,
                text: format!("{}\n", txt),
            });
            t = t2.saturating_add(INTERVAL);
        }
        subs
    }
}

use crate::{hash::Sha2Hash, srt::Subtitles};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::{self, Pointer},
    time::Duration,
};

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

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
pub struct MediaTimestamp(pub Duration);

#[derive(Debug)]
pub struct ContentMetadata {
    pub inner: HashMap<MediaId, ContentData>,
}

#[derive(Debug)]
pub struct RawMediaResults {
    pub content_id: Uuid,
    pub content_name: String,
    pub media: ContentMetadata,
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

#[derive(Debug)]
pub struct ContentSegments {
    pub inner: HashMap<MediaHash, SegmentedVideo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SegmentedVideo {
    pub inner: Vec<(VideoSegmentId, MediaTimestamp)>,
}

impl SegmentedVideo {
    pub fn get_range(&self, start: MediaTimestamp, end: MediaTimestamp) -> Vec<VideoSegmentId> {
        video_range::extract_range(start, end, self.inner.as_slice())
            .cloned()
            .collect()
    }
}

mod video_range {
    use super::MediaTimestamp;
    pub fn extract_range<T>(
        start: MediaTimestamp,
        end: MediaTimestamp,
        data: &[(T, MediaTimestamp)],
    ) -> impl Iterator<Item = &T> {
        if start.0 > end.0 {
            panic!("media start time must be before end time");
        }

        let sres = data.binary_search_by_key(&start, |(_, k)| *k);
        let sidx = match sres {
            Ok(i) => i,
            Err(i) => i - 1,
        };

        let eres = data.binary_search_by_key(&end, |(_, k)| *k);
        let eidx = match eres {
            Ok(i) => i,
            Err(i) => i,
        };

        data[sidx..eidx].iter().map(|(t, _)| t)
    }
    #[cfg(test)]
    mod tests {
        use std::time::Duration;

        use super::*;

        fn ts(s: f32) -> MediaTimestamp {
            MediaTimestamp(Duration::from_secs_f32(s))
        }

        fn build_media(time: f32, segments: usize) -> Vec<(usize, MediaTimestamp)> {
            (0..segments)
                .map(|i| {
                    let start = MediaTimestamp(Duration::from_secs_f32(time * i as f32));
                    (i, start)
                })
                .collect()
        }

        fn extract_to_vec(
            start: MediaTimestamp,
            end: MediaTimestamp,
            data: &[(usize, MediaTimestamp)],
        ) -> Vec<usize> {
            extract_range(start, end, data).cloned().collect()
        }

        #[test]
        fn inside_first_segment() {
            let e = build_media(30.0, 4);
            let v = extract_to_vec(ts(1.0), ts(2.0), e.as_slice());
            assert_eq!(v, vec![0]);
        }
        #[test]
        fn exact_match_middle() {
            let e = build_media(30.0, 4);
            let v = extract_to_vec(ts(30.0), ts(60.0), e.as_slice());
            assert_eq!(v, vec![1]);
        }
        #[test]
        fn span_two_segments() {
            let e = build_media(30.0, 4);
            let v = extract_to_vec(ts(59.0), ts(61.0), e.as_slice());
            assert_eq!(v, vec![1, 2]);
        }
        #[test]
        fn last_segment() {
            let e = build_media(30.0, 4);
            let v = extract_to_vec(ts(100.0), ts(600.0), e.as_slice());
            assert_eq!(v, vec![3]);
        }
        #[test]
        #[should_panic]
        fn invalid_range() {
            let e = build_media(30.0, 4);
            let v = extract_to_vec(ts(100.0), ts(90.0), e.as_slice());
        }
    }
}

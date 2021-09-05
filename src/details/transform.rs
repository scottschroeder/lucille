use super::index::{MediaTimestamp, VideoSegmentId};
use crate::ffmpeg;
use anyhow::Result;
use std::{
    borrow::Cow,
    cell::RefCell,
    path::{Path, PathBuf},
    time::Duration,
};
use tempfile::TempDir;

pub struct SplitResult {
    pub segment: VideoSegmentId,
    pub path: PathBuf,
    pub position: MediaTimestamp,
}

pub trait MediaSplitter: Clone {
    fn chop_into_segments<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SplitResult>>;
}

#[derive(Debug)]
pub struct FFMpegShellSplitter {
    tmpdir: RefCell<Option<tempfile::TempDir>>,
    segment_len: Duration,
}

impl Clone for FFMpegShellSplitter {
    fn clone(&self) -> Self {
        Self {
            tmpdir: RefCell::new(None),
            segment_len: self.segment_len.clone(),
        }
    }
}

impl FFMpegShellSplitter {
    pub fn new(window: Duration) -> FFMpegShellSplitter {
        FFMpegShellSplitter {
            tmpdir: RefCell::new(None),
            segment_len: window,
        }
    }
    pub fn get_tmpdir(&self) -> Result<PathBuf> {
        let mut tmp = self.tmpdir.borrow_mut();
        if tmp.is_none() {
            let new = TempDir::new()?;
            log::trace!("split output dir: {:?}", new);
            *tmp = Some(new);
        }
        Ok(tmp.as_ref().unwrap().path().to_owned())
    }
    pub fn leak(&self) {
        let tmp = self.tmpdir.borrow_mut().take();
        if let Some(t) = tmp {
            log::warn!("leaking tmpdir: {:?}", t.into_path());
        }
    }
}
impl MediaSplitter for FFMpegShellSplitter {
    fn chop_into_segments<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SplitResult>> {
        let path = path.as_ref();
        let video = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("path was not utf-8: {:?}", path))?;

        let tmp = self.get_tmpdir()?;
        let out = tmp
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("path was not utf-8: {:?}", tmp))?;

        let x = ffmpeg::split_media(
            &video,
            &ffmpeg::SplitSettings {
                windows: ffmpeg::SplitStrategy::SegmentTimeSecs(self.segment_len.as_secs_f32()),
            },
            Cow::from(out),
        )?;
        self.leak();
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::details::index::Uuid;

    use super::*;

    #[derive(Clone)]
    struct DummyMediaSplitter {
        episode_len: MediaTimestamp,
        segment_len: Duration,
    }

    impl DummyMediaSplitter {
        pub fn new(episode_len: Duration, segment_len: Duration) -> DummyMediaSplitter {
            DummyMediaSplitter {
                episode_len: MediaTimestamp(episode_len),
                segment_len,
            }
        }
    }

    impl MediaSplitter for DummyMediaSplitter {
        fn chop_into_segments<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SplitResult>> {
            let mut segment_start = MediaTimestamp(Duration::new(0, 0));
            let mut segments = Vec::new();
            while segment_start < self.episode_len {
                let segment_end = std::cmp::min(self.episode_len, {
                    MediaTimestamp(segment_start.0 + self.segment_len)
                });
                segments.push(SplitResult {
                    segment: VideoSegmentId(Uuid::new()),
                    path: PathBuf::new(),
                    position: segment_start,
                });
                segment_start = segment_end;
            }
            Ok(segments)
        }
    }

    #[test]
    fn dummy_splitter_exact() {
        let splitter = DummyMediaSplitter::new(Duration::from_secs(60), Duration::from_secs(30));
        let segments = splitter.chop_into_segments("path").unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].position, MediaTimestamp(Duration::from_secs(0)));
        assert_eq!(
            segments[1].position,
            MediaTimestamp(Duration::from_secs(30))
        );
    }
    #[test]
    fn dummy_splitter_off() {
        let splitter = DummyMediaSplitter::new(Duration::from_secs(55), Duration::from_secs(30));
        let segments = splitter.chop_into_segments("path").unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].position, MediaTimestamp(Duration::from_secs(0)));
        assert_eq!(
            segments[1].position,
            MediaTimestamp(Duration::from_secs(30))
        );
    }
}

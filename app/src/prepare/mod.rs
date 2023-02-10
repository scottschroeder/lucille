use std::{path::PathBuf, time::Duration};

use lucile_core::{media_segment::EncryptionKey, metadata::MediaHash};

mod splitter;
pub use splitter::{MediaSplitter, MediaSplittingStrategy};

#[derive(Debug, thiserror::Error)]
pub enum ProcessingError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Split(#[from] crate::ffmpeg::split::MediaSplitError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessedMedia {
    path: PathBuf,
    hash: MediaHash,
    start: Duration,
    key: Option<EncryptionKey>,
}

#[async_trait::async_trait]
pub trait MediaProcessor {
    async fn process(&self) -> Result<Vec<ProcessedMedia>, ProcessingError>;
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    struct TestProcessor {
        root: tempfile::TempDir,
        output_len: usize,
    }

    impl TestProcessor {
        fn new(output_len: usize) -> TestProcessor {
            TestProcessor {
                root: tempfile::tempdir().unwrap(),
                output_len,
            }
        }
    }

    #[async_trait::async_trait]
    impl MediaProcessor for TestProcessor {
        async fn process(&self) -> Result<Vec<ProcessedMedia>, ProcessingError> {
            let mut s = Duration::default();
            Ok((0..self.output_len)
                .map(|idx| {
                    let data = format!("test_{}", idx);
                    let path = self.root.path().join(&data);
                    let mut f = std::fs::File::create(&path).unwrap();
                    f.write_all(data.as_bytes()).unwrap();
                    let hash = MediaHash::from_bytes(data.as_bytes());
                    let start = s;
                    s = s.saturating_add(Duration::from_secs(30));
                    ProcessedMedia {
                        path,
                        hash,
                        start,
                        key: None,
                    }
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn process_example() {
        let tp = TestProcessor::new(8);
        let media = tp.process().await.unwrap();
        assert_eq!(media.len(), 8);
    }
}

use std::time::Duration;

use super::{MediaProcessor, ProcessedMedia, ProcessingError};
use crate::{
    ffmpeg::{split::FFMpegMediaSplit, FFmpegBinary},
    hashfs::HashFS,
};

pub struct MediaSplittingStrategy<'a> {
    ffmpeg: &'a FFmpegBinary,
    target_duration: Duration,
    target_destination: HashFS,
}

impl<'a> MediaSplittingStrategy<'a> {
    pub fn new<P: Into<std::path::PathBuf>>(
        bin: &'a FFmpegBinary,
        duration: Duration,
        output: P,
    ) -> Result<MediaSplittingStrategy<'a>, std::io::Error> {
        let hash_fs = HashFS::new(output)?;
        Ok(MediaSplittingStrategy {
            ffmpeg: bin,
            target_duration: duration,
            target_destination: hash_fs,
        })
    }
    pub fn split_task(&'a self, src: &'a std::path::Path) -> MediaSplitter<'a> {
        MediaSplitter {
            ffmpeg: self.ffmpeg,
            source: src,
            target_duration: self.target_duration,
            target_destination: &self.target_destination,
        }
    }
}

pub struct MediaSplitter<'a> {
    ffmpeg: &'a FFmpegBinary,
    source: &'a std::path::Path,
    target_duration: Duration,
    target_destination: &'a HashFS,
}

#[async_trait::async_trait]
impl<'a> MediaProcessor for MediaSplitter<'a> {
    async fn process(&self) -> Result<Vec<ProcessedMedia>, ProcessingError> {
        let split = FFMpegMediaSplit::new(self.ffmpeg, self.source, self.target_duration)?;
        let outcome = split.run().await?;
        let mut res = Vec::with_capacity(outcome.records.len());
        for media_split in outcome.records {
            let mut f = tokio::io::BufReader::new(tokio::fs::File::open(media_split.path).await?);
            let (fpath, hash) = self.target_destination.write(&mut f).await?;
            res.push(ProcessedMedia {
                path: fpath,
                hash,
                start: media_split.start,
                key: None,
            })
        }
        Ok(res)
    }
}

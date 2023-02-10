use std::{sync::Arc, time::Duration};

use super::{MediaProcessor, ProcessedMedia, ProcessingError};
use crate::{
    ffmpeg::{
        split::{FFMpegMediaSplit, MediaSplitFile},
        FFmpegBinary,
    },
    hashfs::HashFS,
};

pub struct MediaSplittingStrategy<'a> {
    ffmpeg: &'a FFmpegBinary,
    target_duration: Duration,
    target_destination: Arc<HashFS>,
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
            target_destination: Arc::new(hash_fs),
        })
    }
    pub fn split_task(&'a self, src: &'a std::path::Path) -> MediaSplitter<'a> {
        MediaSplitter {
            ffmpeg: self.ffmpeg,
            source: src,
            target_duration: self.target_duration,
            target_destination: self.target_destination.clone(),
        }
    }
}

pub struct MediaSplitter<'a> {
    ffmpeg: &'a FFmpegBinary,
    source: &'a std::path::Path,
    target_duration: Duration,
    target_destination: Arc<HashFS>,
}

#[async_trait::async_trait]
impl<'a> MediaProcessor for MediaSplitter<'a> {
    async fn process(&self) -> Result<Vec<ProcessedMedia>, ProcessingError> {
        let split = FFMpegMediaSplit::new(self.ffmpeg, self.source, self.target_duration)?;
        let outcome = split.run().await?;
        let mut res = Vec::with_capacity(outcome.records.len());
        let mut set = tokio::task::JoinSet::new();
        for (idx, media_split) in outcome.records.into_iter().enumerate() {
            let fs = self.target_destination.clone();
            set.spawn(async move { handle_split_media(idx, media_split, fs).await });
        }
        while let Some(join_res) = set.join_next().await {
            let m = join_res.map_err(ProcessingError::from).and_then(|r| r)?;
            res.push(m)
        }
        res.sort_by_key(|x| x.idx);
        Ok(res)
    }
}

async fn handle_split_media(
    idx: usize,
    media_split: MediaSplitFile,
    fs: Arc<HashFS>,
) -> Result<ProcessedMedia, ProcessingError> {
    let mut f = tokio::io::BufReader::new(tokio::fs::File::open(media_split.path).await?);
    let (fpath, hash) = fs.write(&mut f).await?;
    Ok(ProcessedMedia {
        idx,
        path: fpath,
        hash,
        start: media_split.start,
        key: None,
    })
}

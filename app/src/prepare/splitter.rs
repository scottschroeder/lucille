use anyhow::Context;
use std::{sync::Arc, time::Duration};

use tokio::io::AsyncReadExt;

use super::{Encryption, MediaProcessor, ProcessedMedia, ProcessingError};
use crate::{
    ffmpeg::{
        split::{FFMpegMediaSplit, MediaSplitFile},
        FFMpegBinary,
    },
    hashfs::HashFS,
};

pub struct MediaSplittingStrategy {
    ffmpeg: FFMpegBinary,
    target_duration: Duration,
    encryption: Encryption,
    target_destination: Arc<HashFS>,
}

impl MediaSplittingStrategy {
    pub fn new<P: Into<std::path::PathBuf>>(
        bin: FFMpegBinary,
        duration: Duration,
        encryption: Encryption,
        output: P,
    ) -> anyhow::Result<MediaSplittingStrategy> {
        let hash_fs = HashFS::new(output)?;
        Ok(MediaSplittingStrategy {
            ffmpeg: bin,
            target_duration: duration,
            encryption,
            target_destination: Arc::new(hash_fs),
        })
    }
    pub fn split_task<'a>(&'a self, src: &'a std::path::Path) -> MediaSplitter<'a> {
        MediaSplitter {
            ffmpeg: &self.ffmpeg,
            source: src,
            target_duration: self.target_duration,
            encryption: self.encryption,
            target_destination: self.target_destination.clone(),
        }
    }
}

pub struct MediaSplitter<'a> {
    ffmpeg: &'a FFMpegBinary,
    source: &'a std::path::Path,
    target_duration: Duration,
    encryption: Encryption,
    target_destination: Arc<HashFS>,
}

#[async_trait::async_trait]
impl<'a> MediaProcessor for MediaSplitter<'a> {
    async fn process(&self) -> anyhow::Result<Vec<ProcessedMedia>> {
        let split = FFMpegMediaSplit::new(self.ffmpeg, self.source, self.target_duration)?;
        let outcome = split.run().await?;
        let mut res = Vec::with_capacity(outcome.records.len());
        let mut set = tokio::task::JoinSet::new();
        for (idx, media_split) in outcome.records.into_iter().enumerate() {
            let fs = self.target_destination.clone();
            let encryption_settings = self.encryption;
            set.spawn(async move {
                handle_split_media(idx, media_split, encryption_settings, fs).await
            });
        }
        while let Some(join_res) = set.join_next().await {
            let m = join_res.context("unable to join tokio task")??;
            res.push(m)
        }
        res.sort_by_key(|x| x.idx);
        Ok(res)
    }
}

async fn handle_split_media(
    idx: usize,
    media_split: MediaSplitFile,
    encryption_settings: Encryption,
    fs: Arc<HashFS>,
) -> anyhow::Result<ProcessedMedia> {
    let (key, ciphertext) = {
        let mut f = tokio::io::BufReader::new(tokio::fs::File::open(media_split.path).await?);
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).await?;
        match encryption_settings {
            Encryption::None => (None, buf),
            Encryption::EasyAes => {
                let (keydata, output) = crate::encryption::easyaes::scramble(&buf)?;
                (Some(keydata), output)
            }
        }
    };
    let mut cursor = std::io::Cursor::new(ciphertext);
    let (fpath, hash) = fs.write(&mut cursor).await?;
    Ok(ProcessedMedia {
        idx,
        path: fpath,
        hash,
        start: media_split.start,
        key,
    })
}

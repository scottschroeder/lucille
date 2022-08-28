use crate::{
    content::{Content, FileSystemContent, MediaHash, VideoFile, VideoSource},
    ffmpeg,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt};
use uuid::Uuid;

pub trait TranscodeClient {
    fn transcode(&self, request: TranscodeRequest) -> Result<TranscodeResponse>;
}

impl<'a, G: GifSink, V: VideoSourceRegistry> TranscodeClient for TranscoderService<'a, V, G> {
    fn transcode(&self, request: TranscodeRequest) -> Result<TranscodeResponse> {
        self.create_gif(request)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipIdentifier {
    pub index: Uuid,
    pub media_hash: MediaHash,
    pub start: usize,
    pub end: usize,
}

impl fmt::Display for ClipIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} {} {} {}",
            self.index, self.media_hash, self.start, self.end
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscodeRequest {
    pub clip: ClipIdentifier,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscodeResponse;

pub trait GifSink {
    fn ffmpeg_arg(&self) -> Cow<'_, str>;
}

pub struct NamedFileOutput(pub String);

impl GifSink for NamedFileOutput {
    fn ffmpeg_arg(&self) -> Cow<'_, str> {
        self.0.as_str().into()
    }
}

pub trait VideoSourceRegistry {
    type V: VideoSource;
    fn select_video(&self, media_hash: MediaHash) -> Self::V;
}

impl VideoSourceRegistry for FileSystemContent {
    type V = VideoFile;

    fn select_video(&self, media_hash: MediaHash) -> Self::V {
        self.videos[&media_hash].clone()
    }
}

pub struct TranscoderService<'a, V, G> {
    id: Uuid,
    content: &'a Content,
    video: &'a V,
    output: &'a G,
}

impl<'a, V, G> TranscoderService<'a, V, G> {
    pub fn new(
        id: Uuid,
        content: &'a Content,
        video: &'a V,
        output: &'a G,
    ) -> TranscoderService<'a, V, G> {
        TranscoderService {
            id,
            content,
            video,
            output,
        }
    }
}

impl<'a, V, G> TranscoderService<'a, V, G>
where
    G: GifSink,
    V: VideoSourceRegistry,
{
    fn create_gif(&self, request: TranscodeRequest) -> Result<TranscodeResponse> {
        let clip = &request.clip;
        if clip.index != self.id {
            anyhow::bail!(
                "the index of the request {:?} does not match the service's index {:?}",
                clip.index,
                self.id
            )
        }

        let episode = self
            .content
            .episodes
            .iter()
            .find(|e| e.media_hash == clip.media_hash)
            .expect("missing episode hash");

        let subs = &episode.subtitles.inner[clip.start..clip.end + 1];
        let output = self.output.ffmpeg_arg();
        let video = self.video.select_video(clip.media_hash);

        ffmpeg::convert_to_gif(&video, subs, output)?;
        Ok(TranscodeResponse)
    }
}

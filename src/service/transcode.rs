use anyhow::Result;
use std::borrow::Cow;
use uuid::Uuid;

use crate::{
    content::{Content, FileSystemContent, VideoFile, VideoSource},
    ffmpeg,
};

pub trait TranscodeClient {
    fn transcode(&self, request: TranscodeRequest) -> Result<TranscodeResponse>;
}

impl<'a, G: GifSink, V: VideoSourceRegistry> TranscodeClient for TranscoderService<'a, V, G> {
    fn transcode(&self, request: TranscodeRequest) -> Result<TranscodeResponse> {
        self.create_gif(request)
    }
}

pub struct ClipIdentifier {
    pub index: Uuid,
    pub episode: usize,
    pub start: usize,
    pub end: usize,
}

pub struct TranscodeRequest {
    pub clip: ClipIdentifier,
}

#[derive(Debug)]
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
    fn select_video(&self, episode: usize) -> Self::V;
}

impl VideoSourceRegistry for FileSystemContent {
    type V = VideoFile;

    fn select_video(&self, episode: usize) -> Self::V {
        self.videos[episode].clone()
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

        let episode = &self.content.episodes[clip.episode];
        let subs = &episode.subtitles[clip.start..clip.end + 1];
        let output = self.output.ffmpeg_arg();
        let video = self.video.select_video(clip.episode);

        ffmpeg::convert_to_gif(&video, subs, output)?;
        Ok(TranscodeResponse)
    }
}

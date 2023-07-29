use std::ops::Range;

use lucille_core::uuid::Uuid;
use serde::{Deserialize, Serialize};

pub use self::make_gif::handle_make_gif_request;
use crate::{app::LucilleApp, ffmpeg::gif::FFMpegCmdAsyncResult};

mod make_gif;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubSegment {
    pub srt_uuid: Uuid,
    pub sub_range: Range<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakeGifRequest {
    pub segments: Vec<SubSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestType {
    MakeGif(MakeGifRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeRequest {
    pub request: RequestType,
}

pub enum TranscodeResponse {
    FFMpegResult(FFMpegCmdAsyncResult),
}

pub async fn handle_transcode_request(
    app: &LucilleApp,
    request: &TranscodeRequest,
) -> anyhow::Result<TranscodeResponse> {
    match &request.request {
        RequestType::MakeGif(gif_request) => {
            let resp = handle_make_gif_request(app, gif_request).await?;
            Ok(TranscodeResponse::FFMpegResult(resp))
        }
    }
}

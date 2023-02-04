use std::ops::Range;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubSegment {
    pub srt_id: i64,
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

use crate::{content::VideoSource};
use std::{borrow::Cow, fmt::Write as fmtWrite, io::Write, process::Command};

pub enum SplitStrategy {
    SegmentTimeSecs(f32)
}

pub struct SplitSettings {
    windows: SplitStrategy,
    width: Option<u32>,
}

// ffmpeg -i ~/ADs01e09.mkv -f segment -segment_time 30 -segment_list out.csv out%03d.mkv
pub fn split_media<S: VideoSource>(
    video: &S,
    settings: &SplitSettings,
    out: Cow<'_, str>,
) -> anyhow::Result<()> {

    let src = video.ffmpeg_src();
    if let Some(t) = video.ffmpeg_type() {
        log::error!("can not deal with source type: {}", t)
    }

    let st = Command::new("ffmpeg")
        .arg("-i")
        .arg(src.as_ref())
        .arg("-y")
        .arg("-f")
        .arg("segment")
        .arg("-segment_time")
        .arg("30")
        .arg("-segment_list")
        .arg("out.csv")
        .arg("out%06d.mkv")
        .status()?;

    if !st.success() {
        anyhow::bail!("ffmpeg failed with exit code: {}", st)
    }

    Ok(())

}
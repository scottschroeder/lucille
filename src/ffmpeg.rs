use crate::{content::VideoSource, srt::Subtitle};
use anyhow::{Context, Result};
use std::{fmt::Write as fmtWrite, io::Write, path, process::Command};
use tempfile::NamedTempFile;

const GIF_DEFAULT_FPS: u32 = 12;
const GIF_DEFAULT_WIDTH: u32 = 480;
const GIF_DEFAULT_FONT: u32 = 28;

pub fn convert_to_gif<S: VideoSource, P: AsRef<path::Path>>(
    video: &S,
    subs: &[Subtitle],
    out: P,
) -> anyhow::Result<()> {
    assert!(!subs.is_empty(), "empty subtitles");

    let new_subs = crate::srt::offset_subs(None, subs);
    let start_time = subs[0].start;
    let end_time = subs[subs.len() - 1].end;
    let elapsed = end_time - start_time;

    let mut srt_file = NamedTempFile::new()?;
    for s in &new_subs {
        writeln!(srt_file, "{}", s)?;
    }

    let srt_file = srt_file.into_temp_path();

    let src = video.ffmpeg_src();
    if let Some(t) = video.ffmpeg_type() {
        log::error!("can not deal with source type: {}", t)
    }

    let mut filter = String::new();

    write!(
        filter,
        "[0:v] fps={},scale=w={}:h=-1, subtitles={}:force_style='Fontsize={}',",
        GIF_DEFAULT_FPS,
        GIF_DEFAULT_WIDTH,
        srt_file.to_str().unwrap(),
        GIF_DEFAULT_FONT
    )?;
    write!(filter,
        "split [a][b];[a] palettegen=stats_mode=single:reserve_transparent=false [p];[b][p] paletteuse=new=1")?;

    let st = Command::new("ffmpeg")
        .arg("-ss")
        .arg(&format!("{:.02}", start_time.as_secs_f32()))
        .arg("-t")
        .arg(&format!("{:.02}", elapsed.as_secs_f32()))
        .arg("-i")
        .arg(src.as_ref())
        .arg("-filter_complex")
        .arg(filter.as_str())
        .arg("-y")
        .arg(out.as_ref())
        .status()?;

    if !st.success() {
        anyhow::bail!("ffmpeg failed with exit code: {}", st)
    }

    Ok(())
}

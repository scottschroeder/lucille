use crate::content::VideoSource;
use std::path::PathBuf;
use std::time::Duration;
use std::{borrow::Cow, fmt::Write as fmtWrite, io::Write, path::Path, process::Command};

const CSV_FILE_NAME: &str = "split_records.csv";

#[derive(Debug)]
pub enum SplitStrategy {
    SegmentTimeSecs(f32),
}

#[derive(Debug)]
pub struct SplitSettings {
    pub windows: SplitStrategy,
}

type Record = (String, f64, f64);

pub fn output_csv_reader<P: AsRef<Path>>(p: P) -> anyhow::Result<Vec<Record>> {
    let f = std::fs::File::open(p)?;
    let mut rdr = csv::Reader::from_reader(f);
    // Instead of creating an iterator with the `records` method, we create
    // an iterator with the `deserialize` method.
    let mut out = Vec::new();
    for result in rdr.deserialize() {
        // We must tell Serde what type we want to deserialize into.
        let record: Record = result?;
        out.push(record);
    }
    Ok(out)
}

// ffmpeg -i ~/ADs01e09.mkv -f segment -segment_time 30 -segment_list out.csv out%03d.mkv
pub fn split_media<S: VideoSource>(
    video: &S,
    settings: &SplitSettings,
    out: Cow<'_, str>,
) -> anyhow::Result<Vec<(PathBuf, Duration)>> {
    let src = video.ffmpeg_src();
    if let Some(t) = video.ffmpeg_type() {
        log::error!("can not deal with source type: {}", t)
    }

    log::info!("Running ffmpeg in {:?} on {:?}: {:?}", out, src, settings);

    let st = Command::new("ffmpeg")
        .current_dir(out.as_ref())
        .arg("-i")
        .arg(src.as_ref())
        .arg("-y")
        .arg("-f")
        .arg("segment")
        .arg("-segment_time")
        .arg("30")
        .arg("-segment_list")
        .arg(CSV_FILE_NAME)
        .arg("out%06d.mkv")
        .status()?;

    if !st.success() {
        anyhow::bail!("ffmpeg failed with exit code: {}", st)
    }

    let out_path = Path::new(out.as_ref());

    let records = output_csv_reader(out_path.join(CSV_FILE_NAME))?;

    let mut segments = Vec::new();
    for (segment_file, start_time_secs, _) in records {
        let segment_path = out_path.join(segment_file);
        let timestamp = Duration::from_secs_f64(start_time_secs);
        segments.push((segment_path, timestamp))
    }

    Ok(segments)
}

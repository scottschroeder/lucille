use std::{
    ffi::OsString,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use super::{FFMpegBinary, FFmpegArg, FFmpegCommand};

const CSV_FILE_NAME: &str = "split_records.csv";

#[derive(Debug, thiserror::Error)]
pub enum MediaSplitError {
    #[error("transcode error, exit {0}")]
    Transcode(i32),
    #[error(transparent)]
    FFMpeg(#[from] std::io::Error),
    #[error(transparent)]
    CSVRead(std::io::Error),
    #[error(transparent)]
    CSVParse(#[from] csv::Error),
}

#[derive(Debug)]
pub(crate) enum OutputDirectory {
    Temp(tempfile::TempDir),
    Path(PathBuf),
}

impl OutputDirectory {
    fn path(&self) -> &std::path::Path {
        match self {
            OutputDirectory::Temp(r) => r.path(),
            OutputDirectory::Path(p) => p.as_path(),
        }
    }
}

#[derive(Debug)]
pub struct MediaSplitFile {
    pub path: PathBuf,
    pub start: Duration,
}

fn ffmpeg_output_to_file_records(root: &Path) -> Result<Vec<MediaSplitFile>, MediaSplitError> {
    Ok(output_csv_reader(root.join(CSV_FILE_NAME).as_path())?
        .into_iter()
        .map(|(name, start, _)| MediaSplitFile {
            path: root.join(name),
            start: Duration::from_secs_f64(start),
        })
        .collect())
}

fn output_csv_reader(p: &Path) -> Result<Vec<FFMpegCSVFormat>, MediaSplitError> {
    let f = std::fs::File::open(p).map_err(MediaSplitError::CSVRead)?;
    parse_csv_to_records(f)
}

type FFMpegCSVFormat = (String, f64, f64);
fn parse_csv_to_records<R: Read>(r: R) -> Result<Vec<FFMpegCSVFormat>, MediaSplitError> {
    let mut rdr = csv::ReaderBuilder::new().has_headers(false).from_reader(r);
    // Instead of creating an iterator with the `records` method, we create
    // an iterator with the `deserialize` method.
    let mut out = Vec::new();
    for result in rdr.deserialize() {
        // We must tell Serde what type we want to deserialize into.
        let record: FFMpegCSVFormat = result?;
        out.push(record);
    }
    Ok(out)
}

#[derive(Debug)]
pub struct FFMpegMediaSplit {
    root: OutputDirectory,
    cmd: FFmpegCommand,
}

#[derive(Debug)]
pub struct FFMpegSplitOutcome {
    root: OutputDirectory,
    pub records: Vec<MediaSplitFile>,
}

impl FFMpegMediaSplit {
    pub fn new<P: Into<OsString>>(
        bin: &FFMpegBinary,
        src: P,
        duration: Duration,
    ) -> Result<FFMpegMediaSplit, MediaSplitError> {
        let root = tempfile::tempdir()?;
        Ok(FFMpegMediaSplit::build_cmd(
            bin.clone(),
            src.into(),
            duration,
            OutputDirectory::Temp(root),
        ))
    }

    pub fn new_with_output<P: Into<OsString>, O: Into<PathBuf>>(
        bin: &FFMpegBinary,
        src: P,
        duration: Duration,
        output: O,
    ) -> Result<FFMpegMediaSplit, MediaSplitError> {
        let output = output.into();
        std::fs::create_dir_all(&output)?;

        Ok(FFMpegMediaSplit::build_cmd(
            bin.clone(),
            src.into(),
            duration,
            OutputDirectory::Path(output),
        ))
    }

    fn build_cmd(
        bin: FFMpegBinary,
        src: OsString,
        duration: Duration,
        root: OutputDirectory,
    ) -> FFMpegMediaSplit {
        // http://underpop.online.fr/f/ffmpeg/help/segment_002c-stream_005fsegment_002c-ssegment.htm.gz

        let mut cmd = bin.build_command();
        cmd.cwd = Some(FFmpegArg::replaced("output_dir", root.path()));

        cmd.args.push(FFmpegArg::plain("-i"));
        cmd.args.push(FFmpegArg::plain(src));
        cmd.args.push(FFmpegArg::plain("-y"));
        cmd.args.push(FFmpegArg::plain("-f"));
        cmd.args.push(FFmpegArg::plain("segment"));
        cmd.args.push(FFmpegArg::plain("-segment_time"));
        cmd.args
            .push(FFmpegArg::plain(format!("{}", duration.as_secs_f32())));
        cmd.args.push(FFmpegArg::plain("-segment_list"));
        cmd.args.push(FFmpegArg::plain(CSV_FILE_NAME));
        cmd.args.push(FFmpegArg::plain("out%06d.mkv"));

        FFMpegMediaSplit { root, cmd }
    }

    pub async fn run(self) -> Result<FFMpegSplitOutcome, MediaSplitError> {
        let FFMpegMediaSplit { root, cmd } = self;
        let mut child = cmd.spawn().await?;
        let exit = child.wait().await?;
        if !exit.success() {
            return Err(MediaSplitError::Transcode(exit.code().unwrap_or(1)));
        }

        let records = ffmpeg_output_to_file_records(root.path())?;
        Ok(FFMpegSplitOutcome { root, records })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV_EXAMPLE: &str = "out000000.mkv,0.000000,32.908000\n\
    out000001.mkv,32.907000,60.727000\n\
    out000002.mkv,60.727000,90.924000";

    #[test]
    fn parse_csv() {
        let csv = std::io::Cursor::new(CSV_EXAMPLE);
        let expected = vec![
            ("out000000.mkv".to_owned(), 0.000000, 32.908000),
            ("out000001.mkv".to_owned(), 32.907000, 60.727000),
            ("out000002.mkv".to_owned(), 60.727000, 90.924000),
        ];
        assert_eq!(parse_csv_to_records(csv).unwrap(), expected);
    }

    #[test]
    fn ffmpeg_split_command() {
        let split = FFMpegMediaSplit::new(
            &FFMpegBinary::default(),
            "video.mkv",
            Duration::from_secs(30),
        )
        .unwrap();
        let actual = format!("{:?}", split.cmd.test_display());
        assert_eq!(
            actual,
            r##"FFMpegTestFormat { bin: "ffmpeg", args: ["-i", "video.mkv", "-y", "-f", "segment", "-segment_time", "30", "-segment_list", "split_records.csv", "out%06d.mkv"], cwd: Some("output_dir"), stdin: None, stdout: None }"##,
        )
    }
}

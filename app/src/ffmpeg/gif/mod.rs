use std::{io, time::Duration};

use lucile_core::Subtitle;
use tokio::io::{AsyncRead, AsyncWriteExt};

use super::{
    cmd::{FFmpegArg, FFmpegCommand},
    FFMpegBinary,
};

const GIF_DEFAULT_FPS: u32 = 12;
const GIF_DEFAULT_WIDTH: u32 = 480;
const GIF_DEFAULT_FONT: u32 = 28;

#[derive(Debug, thiserror::Error)]
pub enum GifTranscodeError {
    #[error("transcode error, exit {0}")]
    Transcode(i32),
    #[error(transparent)]
    FFMpeg(#[from] io::Error),
    #[error("ffmpeg error: exit {}", _0)]
    FFMpegCmd(i32),
    #[error("failure to prepare subtitles")]
    SubtitlePrep(#[source] io::Error),
    #[error(transparent)]
    Tokio(#[from] tokio::task::JoinError),
}

#[derive(Debug)]
pub enum GifType {
    GraphicsInterchangeFormat,
}

impl Default for GifType {
    fn default() -> Self {
        GifType::GraphicsInterchangeFormat
    }
}

impl GifType {
    fn format_name(&self) -> &'static str {
        match self {
            GifType::GraphicsInterchangeFormat => "gif",
        }
    }
}

#[derive(Debug)]
pub struct QualitySettings {
    pub fps: u32,
    pub width: u32,
}

impl Default for QualitySettings {
    fn default() -> Self {
        Self {
            fps: GIF_DEFAULT_FPS,
            width: GIF_DEFAULT_WIDTH,
        }
    }
}

#[derive(Debug)]
pub struct GifSettings {
    pub media_type: GifType,
    pub quality: QualitySettings,
    pub font_size: u32,
    pub cut_selection: GifTimeSelection,
}

impl Default for GifSettings {
    fn default() -> Self {
        Self {
            media_type: Default::default(),
            quality: Default::default(),
            font_size: GIF_DEFAULT_FONT,
            cut_selection: Default::default(),
        }
    }
}

/// Options for choosing how to cut the Gif
/// The default is `Relative(0)`, or exactly at the start/stop
/// of the subtitles
#[derive(Debug)]
pub enum CutSetting {
    /// The exact time in the video stream to cut
    Exact(Duration),
    /// How much time to leave around the start/end.
    Relative(Duration),
}

impl Default for CutSetting {
    fn default() -> Self {
        CutSetting::Relative(Duration::default())
    }
}

#[derive(Debug, Default)]
pub struct GifTimeSelection {
    pub start: CutSetting,
    pub end: CutSetting,
}

#[derive(Debug)]
pub struct FFMpegGifTranscoder {
    root: tempfile::TempPath,
    media: tempfile::NamedTempFile,
    cmd: FFmpegCommand,
}

impl FFMpegGifTranscoder {
    pub async fn build_cmd(
        bin: FFMpegBinary,
        subs: &[Subtitle],
        settings: &GifSettings,
    ) -> Result<FFMpegGifTranscoder, GifTranscodeError> {
        let srt = tempfile::NamedTempFile::new().map_err(GifTranscodeError::SubtitlePrep)?;
        let media_tmp = tempfile::NamedTempFile::new().map_err(GifTranscodeError::SubtitlePrep)?;

        let (tmp_file, tmp_path) = srt.into_parts();
        let path_arg = tmp_path.to_str().ok_or_else(|| {
            GifTranscodeError::SubtitlePrep(io::Error::new(
                io::ErrorKind::Other,
                "path was not utf8",
            ))
        })?;
        let media_path_arg = media_tmp.path().to_str().ok_or_else(|| {
            GifTranscodeError::SubtitlePrep(io::Error::new(
                io::ErrorKind::Other,
                "path was not utf8",
            ))
        })?;

        let mut f = tokio::fs::File::from_std(tmp_file);
        for sub in subs {
            let s = format!("{}", sub);
            f.write_all(s.as_bytes())
                .await
                .map_err(GifTranscodeError::SubtitlePrep)?;
        }

        let mut cmd = bin.build_command();

        let (start, end) = get_cut_times(subs, &settings.cut_selection);
        // TODO ss if we have the file, `-s` if we only have a stream?
        // cmd.args.push(FFmpegArg::plain("-s"));
        cmd.args.push(FFmpegArg::plain("-ss"));
        cmd.args.push(FFmpegArg::plain(format!("{:.02}", start)));
        cmd.args.push(FFmpegArg::plain("-t"));
        cmd.args.push(FFmpegArg::plain(format!("{:.02}", end)));

        // cmd.args.push(FFmpegArg::plain("-f"));
        // cmd.args.push(FFmpegArg::plain("h264"));
        // cmd.args.push(FFmpegArg::plain("hevc"));

        cmd.args.push(FFmpegArg::plain("-i"));
        cmd.args.push(FFmpegArg::plain(media_path_arg));
        // cmd.args.push(FFmpegArg::plain("pipe:0"));

        let filter = create_filter(settings, path_arg).map_err(|e| {
            GifTranscodeError::SubtitlePrep(io::Error::new(io::ErrorKind::Other, e))
        })?;
        cmd.args.push(FFmpegArg::plain("-filter_complex"));
        cmd.args.push(FFmpegArg::plain(filter));

        cmd.args.push(FFmpegArg::plain("-f"));
        cmd.args
            .push(FFmpegArg::plain(settings.media_type.format_name()));

        // cmd.args.push(FFmpegArg::plain("-"));
        cmd.args.push(FFmpegArg::plain("ffout.gif"));

        cmd.stdin = Some(super::cmd::StdIo::Piped);
        cmd.stdout = Some(super::cmd::StdIo::Piped);

        Ok(FFMpegGifTranscoder {
            root: tmp_path,
            media: media_tmp,
            cmd,
        })
    }

    /// launch ffmpeg in the background, returns a handle to the task
    /// as well as a reader for stdout.
    ///
    /// YOU MUST CONSUME STDOUT BEFORE `await` ON THE HANDLE
    pub async fn launch(
        self,
        mut input: Box<dyn AsyncRead + Unpin + Send>,
        // mut input: impl AsyncRead + Unpin + Send,
    ) -> Result<(FFMpegCmdAsyncResult, impl tokio::io::AsyncRead), GifTranscodeError> {
        let tmp = self.root;
        let (media_file, media_tmp_path) = self.media.into_parts();
        let mut media_file = tokio::fs::File::from_std(media_file);

        log::debug!("copy input to {:?}", media_tmp_path);
        tokio::io::copy(&mut input, &mut media_file).await?;

        let mut handle = self.cmd.spawn().await?;
        let mut stdin = handle.stdin.take().unwrap();
        stdin.shutdown().await?;

        let stdout = handle.stdout.take().unwrap();

        let cmd_result = tokio::task::spawn(async move {
            // let (copy_result, wait_result) =
            //     tokio::join!(tokio::io::copy(&mut input, &mut stdin), handle.wait(),);
            // wait_result.and_then(|e| copy_result.map(|_| e))
            handle.wait().await
        });
        let result = FFMpegCmdAsyncResult {
            inner: cmd_result,
            _tmpfile: (tmp, media_tmp_path),
        };

        Ok((result, stdout))
    }
}

pub struct FFMpegCmdAsyncResult {
    inner: tokio::task::JoinHandle<Result<std::process::ExitStatus, std::io::Error>>,
    _tmpfile: (tempfile::TempPath, tempfile::TempPath),
}

impl FFMpegCmdAsyncResult {
    pub async fn check(self) -> Result<(), GifTranscodeError> {
        let st = self.inner.await??;
        log::trace!("ffmpeg complete: {:?}", st);
        if st.success() {
            Ok(())
        } else if let Some(code) = st.code() {
            Err(GifTranscodeError::FFMpegCmd(code))
        } else {
            Err(GifTranscodeError::FFMpegCmd(-1))
        }
    }
}

fn get_cut_times(subs: &[Subtitle], cut_selection: &GifTimeSelection) -> (f32, f32) {
    let start_time = match cut_selection.start {
        CutSetting::Exact(t) => t,
        CutSetting::Relative(t) => subs
            .first()
            .map(|s| s.start)
            .unwrap_or_default()
            .saturating_sub(t),
    };
    let end_time = match cut_selection.end {
        CutSetting::Exact(t) => t,
        CutSetting::Relative(t) => subs
            .last()
            .map(|s| s.end)
            .unwrap_or_default()
            .saturating_add(t),
    };

    let s = start_time.as_secs_f32();
    let e = end_time
        .checked_sub(start_time)
        .expect("it should not be possible for the end time to be before the start time")
        .as_secs_f32();
    (s, e)
}

fn create_filter(settings: &GifSettings, srt_file: &str) -> Result<String, std::fmt::Error> {
    use std::fmt::Write;
    let mut filter = String::new();

    write!(filter, "fps={}", settings.quality.fps)?;
    write!(filter, ",scale=w={}:h=-1", settings.quality.width)?;
    // write!(
    //     filter,
    //     ",subtitles={}:force_style='Fontsize={}'",
    //     srt_file, settings.font_size,
    // )?;
    // filter.push(',');
    // filter.push_str(
    //     "split [a][b];[a] palettegen=stats_mode=single:reserve_transparent=false [p];[b][p] paletteuse=new=1"
    //     );
    Ok(filter)
}

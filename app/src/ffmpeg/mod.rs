mod cmd;
pub mod gif;
pub mod split;

pub use cmd::FFMpegBinary;
use cmd::{FFmpegArg, FFmpegCommand};

#[derive(Debug, thiserror::Error)]
#[deprecated(note = "use anyhow")]
pub enum FFmpegError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

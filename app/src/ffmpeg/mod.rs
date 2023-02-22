mod cmd;
pub mod split;

pub use cmd::FFMpegBinary;
use cmd::{FFmpegArg, FFmpegCommand};

#[derive(Debug, thiserror::Error)]
pub enum FFmpegError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

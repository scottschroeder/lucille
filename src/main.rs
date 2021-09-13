#![feature(binary_heap_into_iter_sorted)]
use anyhow::Result;

mod cli_select;
mod content;
mod error;
mod ffmpeg;
mod search;
mod service;
mod srt;
mod srt_loader;
mod storage;
mod details {
    mod encrypted;
    mod index;
    pub mod process;
    pub mod storage;
    pub mod transform;
    pub use index::{ContentData, MediaHash, MediaId, SegmentedVideo};
    pub use storage::Storage;
    pub use transform::MediaSplitter;
}
mod cli;
mod hash;

/*
    TODO:
    - async file copies? two encoders?
    - encrypted storage
    - reload storage
    - multiple prepare varieties?
*/

fn main() -> Result<()> {
    color_backtrace::install();
    cli::run_cli().map_err(|e| {
        log::error!("{}", e);
        e.chain()
            .skip(1)
            .for_each(|cause| log::error!("because: {}", cause));
        anyhow::anyhow!("unrecoverable lucile failure")
    })
}

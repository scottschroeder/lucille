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
    pub mod storage;
    pub mod transform;
    pub mod process;
}

mod cli;

fn main() -> Result<()> {
    color_backtrace::install();
    cli::run_cli()
}



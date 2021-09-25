#![feature(binary_heap_into_iter_sorted)]

mod cli;
mod cli_select;
mod content;
mod error;
mod ffmpeg;
mod hash;
mod search;
mod service;
mod srt;
mod storage;

fn main() -> anyhow::Result<()> {
    color_backtrace::install();
    let args = cli::argparse::get_args();
    cli::setup_logger(args.occurrences_of("verbosity"));
    log::trace!("Args: {:?}", args);

    cli::run_cli(&args).map_err(|e| {
        log::error!("{}", e);
        e.chain()
            .skip(1)
            .for_each(|cause| log::error!("because: {}", cause));
        anyhow::anyhow!("unrecoverable lucile failure")
    })
}

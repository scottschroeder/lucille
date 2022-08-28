mod cli;
mod content;
mod ffmpeg;
mod search;
mod service;
mod srt;
mod storage;

// fn main() -> anyhow::Result<()> {
//     color_backtrace::install();
//     let args = cli::argparse::get_args();
//     cli::setup_logger(args.occurrences_of("verbosity"));
//     log::trace!("Args: {:?}", args);

//     cli::run_cli(&args).map_err(|e| {
//         log::error!("{}", e);
//         e.chain()
//             .skip(1)
//             .for_each(|cause| log::error!("because: {}", cause));
//         anyhow::anyhow!("unrecoverable lucile failure")
//     })
// }

pub fn setup_logger(level: u8) {
    let mut builder = pretty_env_logger::formatted_timed_builder();

    let noisy_modules: &[&str] = &[];

    let log_level = match level {
        //0 => log::Level::Error,
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    if level > 1 && level < 4 {
        for module in noisy_modules {
            builder.filter_module(module, log::LevelFilter::Info);
        }
    }

    builder.filter_level(log_level);
    builder.format_timestamp_millis();
    builder.init();
}

fn main() -> anyhow::Result<()> {
    color_backtrace::install();
    let args = cli::argparse::get_args();
    setup_logger(args.verbose);
    log::trace!("Args: {:?}", args);

    match &args.subcmd {
        cli::argparse::SubCommand::Media(sub) => dummy(),
        _ => dummy(), // argparse::SubCommand::Test(sub) => run_test(sub),
    }
    .map_err(|e| {
        log::error!("{:?}", e);
        anyhow::anyhow!("unrecoverable {} failure", clap::crate_name!())
    })
}

fn dummy() -> anyhow::Result<()> {
    Ok(())
}

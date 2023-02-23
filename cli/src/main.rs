mod cli;
// mod content;
// mod ffmpeg;
// mod search;
// mod service;
// mod srt;
// mod storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    color_backtrace::install();
    let args = cli::get_args();
    setup_logger(args.verbose);
    log::trace!("Args: {:?}", args);

    args.run().await.map_err(|e| {
        log::error!("{:?}", e);
        anyhow::anyhow!("unrecoverable {} failure", clap::crate_name!())
    })
}

pub fn setup_logger(level: u8) {
    let mut builder = pretty_env_logger::formatted_timed_builder();

    let noisy_modules: &[&str] = &["sqlx::query", "tantivy::directory::mmap_directory"];

    let log_level = match level {
        //0 => log::Level::Error,
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    if level > 1 && level < 4 {
        for module in noisy_modules {
            builder.filter_module(module, log::LevelFilter::Warn);
        }
    }

    builder.filter_level(log_level);
    builder.format_timestamp_millis();
    builder.init();
}

use anyhow::Result;

mod error;
mod search;
mod srt;
mod srt_loader;
mod storage;

const STORAGE_DEFAULT: &str = "storage";

fn main() -> Result<()> {
    color_backtrace::install();
    let args = cli::get_args();
    setup_logger(args.occurrences_of("verbosity"));
    log::trace!("Args: {:?}", args);

    match args.subcommand() {
        ("test", Some(sub_m)) => test_fn(sub_m),
        ("", _) => Err(anyhow::anyhow!(
            "Please provide a command:\n{}",
            args.usage()
        )),
        subc => Err(anyhow::anyhow!(
            "Unknown command: {:?}\n{}",
            subc,
            args.usage()
        )),
    }
    .map_err(|e| {
        log::error!("{:?}", e);
        anyhow::anyhow!("unrecoverable lucile failure")
    })
}

fn test_fn(args: &clap::ArgMatches) -> Result<()> {
    let q = args.value_of("query").unwrap_or("default");
    let storage_path = std::path::Path::new(STORAGE_DEFAULT);

    let s = storage::Storage::load(storage_path).or_else(|_| {
        let eps = srt_loader::parse_adsubs()?;
        storage::Storage::build_index(storage_path, eps)
    })?;

    let r = search::search(&s.index, q, &s.episodes).map_err(error::TError::from)?;
    // log::info!("{:#?}", r);
    // let mut count = 0;
    // for e in &eps {
    //     for c in e.slices(5) {
    //         // log::debug!("[{}, {}]: {:?}", c.start, c.end, c.text);
    //         count += 1;
    //     }
    // }
    // println!("{}", count);

    Ok(())
}

fn setup_logger(level: u64) {
    let mut builder = pretty_env_logger::formatted_timed_builder();

    let noisy_modules = &[
        "hyper",
        "mio",
        "tokio_core",
        "tokio_reactor",
        "tokio_threadpool",
        "fuse::request",
        "rusoto_core",
        "want",
        "tantivy",
    ];

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
    //builder.format(|buf, record| writeln!(buf, "{}", record.args()));
    builder.init();
}

mod cli {
    use clap::SubCommand;
    pub fn get_args() -> clap::ArgMatches<'static> {
        clap::App::new(clap::crate_name!())
            .version(clap::crate_version!())
            .about(clap::crate_description!())
            .setting(clap::AppSettings::DeriveDisplayOrder)
            .arg(
                clap::Arg::with_name("verbosity")
                    .short("v")
                    .multiple(true)
                    .global(true)
                    .help("Sets the level of verbosity"),
            )
            .subcommand(
                SubCommand::with_name("test")
                    .arg(
                        clap::Arg::with_name("query")
                            .long("query")
                            .takes_value(true),
                    )
                    .arg(clap::Arg::with_name("flag").long("flag")),
            )
            .get_matches()
    }
}

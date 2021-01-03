#![feature(binary_heap_into_iter_sorted)]
use anyhow::Result;
use service::search::{SearchClient, SearchRequest, SearchService};

mod cli_select;
mod content;
mod error;
mod ffmpeg;
mod search;
mod service;
mod srt;
mod srt_loader;
mod storage;

const STORAGE_DEFAULT: &str = "storage";
const INDEX_WINDOW_DEFAULT: &str = "5";
const OUTPUT_DEFAULT: &str = "out.gif";

fn main() -> Result<()> {
    color_backtrace::install();
    let args = cli::get_args();
    setup_logger(args.occurrences_of("verbosity"));
    log::trace!("Args: {:?}", args);

    match args.subcommand() {
        ("interactive", Some(sub_m)) => interactive(sub_m),
        ("index", Some(sub_m)) => index(sub_m),
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

fn index(args: &clap::ArgMatches) -> Result<()> {
    let content_path = args.value_of("path").unwrap();
    let storage_path = args.value_of("storage").unwrap();
    let max_window = args.value_of("index_window").unwrap().parse::<usize>()?;
    let storage_path = std::path::Path::new(storage_path);

    std::fs::remove_dir_all(storage_path)?;

    let (content, videos) = content::scan::scan_filesystem(content_path)?;
    let s = storage::Storage::new(storage_path);
    let _index = s.build_index(content, videos, max_window)?;
    Ok(())
}

fn interactive(args: &clap::ArgMatches) -> Result<()> {
    let output = args.value_of("output_gif").unwrap();
    let storage_path = args.value_of("storage").unwrap();
    let storage_path = std::path::Path::new(storage_path);
    let s = storage::Storage::new(storage_path);

    let req = SearchRequest {
        query: args.value_of("query").unwrap(),
        window: args
            .value_of("search_window")
            .map(|s| s.parse::<usize>())
            .transpose()?,
        max_responses: Some(5),
    };

    let db = s.load()?;
    let index = s.index()?;
    let search_service = SearchService::new(db.id, index, &db.content);

    let resp = search_service.search(req)?;

    let clip = cli_select::ask_user_for_clip(&db.content, &resp)?;

    let episode = &db.content.episodes[clip.episode];
    let subs = &episode.subtitles[clip.start..clip.end + 1];
    let video = &db.videos.videos[clip.episode];

    ffmpeg::convert_to_gif(video, subs, output)?;

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

    use crate::{INDEX_WINDOW_DEFAULT, OUTPUT_DEFAULT, STORAGE_DEFAULT};
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
                SubCommand::with_name("index")
                    .arg(
                        clap::Arg::with_name("path")
                            .long("path")
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        clap::Arg::with_name("storage")
                            .long("storage")
                            .default_value(STORAGE_DEFAULT)
                            .takes_value(true),
                    )
                    .arg(
                        clap::Arg::with_name("index_window")
                            .long("max-window")
                            .default_value(INDEX_WINDOW_DEFAULT)
                            .takes_value(true),
                    ),
            )
            .subcommand(
                SubCommand::with_name("interactive")
                    .arg(
                        clap::Arg::with_name("query")
                            .long("query")
                            .takes_value(true),
                    )
                    .arg(
                        clap::Arg::with_name("storage")
                            .long("storage")
                            .default_value(STORAGE_DEFAULT)
                            .takes_value(true),
                    )
                    .arg(
                        clap::Arg::with_name("search_window")
                            .long("search-window")
                            .takes_value(true),
                    )
                    .arg(
                        clap::Arg::with_name("index_window")
                            .long("max-window")
                            .takes_value(true),
                    )
                    .arg(
                        clap::Arg::with_name("output_gif")
                            .long("out")
                            .short("o")
                            .default_value(OUTPUT_DEFAULT)
                            .takes_value(true),
                    ),
            )
            .get_matches()
    }
}

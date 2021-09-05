#![feature(binary_heap_into_iter_sorted)]
use std::time::Duration;

use anyhow::Result;
use service::{
    search::{SearchClient, SearchRequest, SearchService},
    transcode::{
        ClipIdentifier, NamedFileOutput, TranscodeClient, TranscodeRequest, TranscoderService,
    },
};
use storage::Storage;

use crate::content::scan::scan_filesystem;

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

const STORAGE_DEFAULT: &str = "storage";
const INDEX_WINDOW_DEFAULT: &str = "5";
const OUTPUT_DEFAULT: &str = "out.gif";

fn main() -> Result<()> {
    color_backtrace::install();
    let args = cli::get_args();
    setup_logger(args.occurrences_of("verbosity"));
    log::trace!("Args: {:?}", args);

    match args.subcommand() {
        ("scan-titles", Some(sub_m)) => scan_titles(sub_m),
        ("interactive", Some(sub_m)) => interactive(sub_m),
        ("search", Some(sub_m)) => search(sub_m),
        ("transcode", Some(sub_m)) => transcode(sub_m),
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

fn get_storage(args: &clap::ArgMatches) -> Storage {
    let storage_path = args.value_of("storage").unwrap();
    let storage_path = std::path::Path::new(storage_path);
    Storage::new(storage_path)
}

fn get_search_request<'a>(args: &'a clap::ArgMatches) -> Result<SearchRequest<'a>> {
    Ok(SearchRequest {
        query: args.value_of("query").unwrap(),
        window: args
            .value_of("search_window")
            .map(|s| s.parse::<usize>())
            .transpose()?,
        max_responses: Some(5),
    })
}

fn parse_spec_shorthand(mut spec: clap::Values) -> Result<ClipIdentifier> {
    let id = spec.next().ok_or_else(|| anyhow::anyhow!("no id"))?;
    let ep = spec.next().ok_or_else(|| anyhow::anyhow!("no episode"))?;
    let start = spec.next().ok_or_else(|| anyhow::anyhow!("no start"))?;
    let end = spec.next().ok_or_else(|| anyhow::anyhow!("no end"))?;

    Ok(ClipIdentifier {
        index: uuid::Uuid::parse_str(id)?,
        episode: ep.parse::<usize>()?,
        start: start.parse::<usize>()?,
        end: end.parse::<usize>()?,
    })
}

fn search(args: &clap::ArgMatches) -> Result<()> {
    let s = get_storage(args);

    let db = s.load()?;
    let index = s.index()?;
    let search_service = SearchService::new(db.id, index, &db.content);

    let search_request = get_search_request(args)?;

    let search_response = search_service.search(search_request)?;
    println!("{}", serde_json::to_string_pretty(&search_response)?);

    Ok(())
}

fn transcode(args: &clap::ArgMatches) -> Result<()> {
    let spec = args.values_of("spec").unwrap();
    let output = args.value_of("output_gif").unwrap();
    let s = get_storage(args);

    let clip = parse_spec_shorthand(spec)?;
    let db = s.load()?;
    let gif_output = NamedFileOutput(output.to_string());
    let transcode_service = TranscoderService::new(db.id, &db.content, &db.videos, &gif_output);

    let transcode_request = TranscodeRequest { clip };
    let transcode_response = transcode_service.transcode(transcode_request)?;

    println!("{:?}", transcode_response);
    Ok(())
}

fn scan_titles(args: &clap::ArgMatches) -> Result<()> {
    let p = std::path::Path::new(args.value_of("path").unwrap());
    log::debug!("scan titles: {:?}", p);
    let (content, fs_content) = scan_filesystem(p)?;
    let (media , files)= crate::details::process::intake_media(content, fs_content);
    log::debug!("{:#?}", media);
    log::debug!("{:#?}", files);

    // TODO WHERE I LEFT OFF
    let fs = crate::details::storage::FileStorage::new("storage_backend")?;
    let splitter = crate::details::transform::FFMpegShellSplitter::new(Duration::from_secs(30));
    let x = crate::details::process::split_media(&fs, &splitter, files)?;
    log::debug!("{:#?}", x);


    Ok(())
}

fn interactive(args: &clap::ArgMatches) -> Result<()> {
    let output = args.value_of("output_gif").unwrap();
    let s = get_storage(args);

    let db = s.load()?;
    let index = s.index()?;
    let search_service = SearchService::new(db.id, index, &db.content);
    let gif_output = NamedFileOutput(output.to_string());
    let transcode_service = TranscoderService::new(db.id, &db.content, &db.videos, &gif_output);

    let search_request = get_search_request(args)?;

    let search_response = search_service.search(search_request)?;
    let clip = cli_select::ask_user_for_clip(&db.content, &search_response)?;
    let transcode_request = TranscodeRequest { clip };
    let transcode_response = transcode_service.transcode(transcode_request)?;

    println!("{:?}", transcode_response);

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
                SubCommand::with_name("search")
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
                    ),
            )
            .subcommand(
                SubCommand::with_name("transcode")
                    .arg(clap::Arg::with_name("spec").multiple(true))
                    .arg(
                        clap::Arg::with_name("storage")
                            .long("storage")
                            .default_value(STORAGE_DEFAULT)
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
            .subcommand(
                SubCommand::with_name("scan-titles").arg(
                    clap::Arg::with_name("path")
                        .required(true)
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

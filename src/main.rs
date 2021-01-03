#![feature(binary_heap_into_iter_sorted)]
use anyhow::{Context, Result};

mod content;
mod error;
mod ffmpeg;
mod search;
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
        ("test", Some(sub_m)) => test_fn(sub_m),
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

fn test_fn(args: &clap::ArgMatches) -> Result<()> {
    let q = args.value_of("query").unwrap_or("default");
    let output = args.value_of("output_gif").unwrap();
    let search_window = args
        .value_of("search_window")
        .unwrap_or("5")
        .parse::<usize>()?;
    let storage_path = std::path::Path::new(STORAGE_DEFAULT);

    let s = storage::Storage::new(storage_path);
    log::trace!("searching...");
    let scores = search::search(&s.index()?, q, search_window).map_err(error::TError::from)?;
    log::trace!("ranking...");
    let ranked = search::rank(&scores, 5);
    let content = s.content()?;
    search::print_top_scores(&content, &ranked);
    let input = get_user_input("make a selection: e.g. 'B 3-5'")?;
    let (index, start, end) = parse_user_selection(input.as_str())?;

    let choice = &ranked[index];
    let episode = &content.episodes[choice.ep];
    let e_start = choice.clip.index + start;
    let e_end = choice.clip.index + end + 1;
    let subs = &episode.subtitles[e_start..e_end];

    let video = &s.videos()?.videos[choice.ep];

    // ffmpeg_cmd(video, subs)?;
    ffmpeg::convert_to_gif(video, subs, output)?;

    Ok(())
}

fn get_user_input(msg: &str) -> anyhow::Result<String> {
    println!("{}", msg);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(input)
}

fn parse_user_selection(s: &str) -> anyhow::Result<(usize, usize, usize)> {
    let re =
        once_cell_regex::regex!(r##" *(?P<letter>[a-zA-Z]) *(?P<start>[0-9]+)\-(?P<end>[0-9]+)"##);
    let captures = re
        .captures(s)
        .ok_or_else(|| anyhow::anyhow!("could not parse user selection"))?;
    let letter = captures
        .name("letter")
        .expect("non optional regex match")
        .as_str()
        .chars()
        .next()
        .ok_or_else(|| anyhow::anyhow!("string did not contain letter?"))?;
    let start = captures
        .name("start")
        .expect("non optional regex match")
        .as_str()
        .parse::<usize>()
        .with_context(|| "unable to parse digits")?;
    let end = captures
        .name("end")
        .expect("non optional regex match")
        .as_str()
        .parse::<usize>()
        .with_context(|| "unable to parse digits")?;

    let user_choice_index = match letter {
        'a'..='z' => (letter as u8) - 'a' as u8,
        'A'..='Z' => (letter as u8) - 'A' as u8,
        _ => anyhow::bail!("invalid char: {:?}", letter),
    } as usize;

    Ok((user_choice_index, start, end))
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
                SubCommand::with_name("test")
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
                    )
                    .arg(clap::Arg::with_name("flag").long("flag")),
            )
            .get_matches()
    }
}

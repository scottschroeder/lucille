#![feature(binary_heap_into_iter_sorted)]
use anyhow::{Context, Result};
use srt::Subtitle;

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
    let max_window = args
        .value_of("index_window")
        .unwrap_or("5")
        .parse::<usize>()?;
    let search_window = args
        .value_of("search_window")
        .unwrap_or("5")
        .parse::<usize>()?;
    let storage_path = std::path::Path::new(STORAGE_DEFAULT);

    let s = storage::Storage::load(storage_path).or_else(|_| {
        let eps = srt_loader::parse_adsubs()?;
        storage::Storage::build_index(storage_path, eps, max_window)
    })?;

    let scores =
        search::search(&s.index, q, &s.episodes, search_window).map_err(error::TError::from)?;
    let ranked = search::rank(&scores, 5);
    search::print_top_scores(&s.episodes, &ranked);
    let input = get_user_input("make a selection: e.g. 'B 3-5'")?;
    let (index, start, end) = parse_user_selection(input.as_str())?;

    let choice = &ranked[index];
    let episode = &s.episodes[choice.ep];
    let e_start = choice.clip.index + start;
    let e_end = choice.clip.index + end + 1;
    let s_start = &episode.subs[e_start];
    let s_end = &episode.subs[e_end];
    let subs = &episode.subs[e_start..e_end];
    // println!(
    //     "{}: {:?}-{:?}\n{}",
    //     episode.title,
    //     s_start.start,
    //     s_end.end,
    //     episode.extract_window(e_start, e_end)
    // );

    ffmpeg_cmd(subs)?;

    Ok(())
}

fn ffmpeg_cmd(subs: &[Subtitle]) -> anyhow::Result<()> {
    use std::io::Write;
    assert!(!subs.is_empty());
    let new_subs = crate::srt::offset_subs(None, subs);
    let start_time = subs[0].start;
    let end_time = subs[subs.len() - 1].end;
    let elapsed = end_time - start_time;

    let subs_file = "tmp.srt";
    let _ = std::fs::remove_file(subs_file);
    let mut f = std::fs::File::create(subs_file)?;

    for s in &new_subs {
        writeln!(f, "{}", s)?;
    }

    let fps = 12;
    let width = 480;
    let font_size = 28;

    print!(
        "ffmpeg -ss {:.02} -t {:.02} -i INPUT -filter_complex ",
        start_time.as_secs_f32(),
        elapsed.as_secs_f32()
    );
    print!(
        "\"[0:v] fps={},scale=w={}:h=-1, subtitles={}:force_style='Fontsize={}',",
        fps, width, subs_file, font_size
    );
    print!("split [a][b];[a] palettegen=stats_mode=single:reserve_transparent=false [p];[b][p] paletteuse=new=1\"");
    println!(" -y out.gif");
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
                    .arg(clap::Arg::with_name("flag").long("flag")),
            )
            .get_matches()
    }
}

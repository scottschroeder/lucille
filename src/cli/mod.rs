use anyhow::Result;

pub mod argparse;
mod helpers;
mod media_intake;

pub fn run_cli(args: &clap::ArgMatches) -> Result<()> {
    match args.subcommand() {
        ("media", Some(sub_m)) => match sub_m.subcommand() {
            ("scan", Some(sub_m)) => media_intake::scan_titles(sub_m),
            ("index", Some(sub_m)) => media_intake::index(sub_m),
            ("prepare", Some(sub_m)) => media_intake::prepare(sub_m),
            ("", _) => Err(anyhow::anyhow!(
                "Please provide a command:\n{}",
                args.usage()
            )),
            subc => Err(anyhow::anyhow!(
                "Unknown command: {:?}\n{}",
                subc,
                args.usage()
            )),
        },
        ("scan-titles", Some(sub_m)) => media_intake::scan_titles(sub_m),
        ("demo", Some(sub_m)) => demo(sub_m),
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
}

fn demo(_args: &clap::ArgMatches) -> Result<()> {
    // details::encrypted::aesbytes::encrypt("lksjdfsdforiuweoriuweroiuwecwlkj");
    Ok(())
}

pub fn setup_logger(level: u64) {
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

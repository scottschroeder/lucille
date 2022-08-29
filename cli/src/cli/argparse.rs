use clap::Parser;

const STORAGE_DEFAULT: &str = "storage";
const INDEX_WINDOW_DEFAULT: &str = "5";
const OUTPUT_DEFAULT: &str = "out.gif";
pub fn get_args() -> CliOpts {
    CliOpts::parse()
}

#[derive(Parser, Debug)]
#[clap(version = clap::crate_version!(), author = "Scott S. <scottschroeder@sent.com>")]
pub struct CliOpts {
    #[clap(short, long, global = true, parse(from_occurrences))]
    pub verbose: u8,
    #[clap(subcommand)]
    pub subcmd: SubCommand,
}

#[derive(Parser, Debug)]
pub enum SubCommand {
    /// Create a new corpus
    #[clap(subcommand)]
    Corpus(CorpusCommand),
    // Process and prepare media
    // #[clap(subcommand)]
    // Media(MediaCommand),
    // Index(IndexCommand),

    // Library(LibraryCommand),
    // Search(PrepareCommand),
    // Transcode(PrepareCommand),
    // Test(PrepareCommand),
    // Interactive(PrepareCommand),
    // Render an image
    // #[clap(subcommand)]
    // Render(Render),
}

#[derive(Parser, Debug)]
pub struct IndexCommand {}
#[derive(Parser, Debug)]
pub struct PrepareCommand {}

#[derive(Parser, Debug)]
pub enum CorpusCommand {
    /// Create a new corpus
    New(CorpusNewOpts),
    /// List existing corpuses
    List(CorpusListOpts),
}

#[derive(Parser, Debug)]
pub struct CorpusNewOpts {
    pub name: String,
    #[clap(flatten)]
    pub db: DatabaseConfig,
}

#[derive(Parser, Debug)]
pub struct CorpusListOpts {
    #[clap(flatten)]
    pub db: DatabaseConfig,
}

#[derive(Parser, Debug)]
pub struct DatabaseConfig {
    /// Path to sqlite database file.
    /// 
    /// If not provided, will attempt to read `DATABASE_URL` env var.
    #[clap(long)]
    pub database_path: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub enum MediaCommand {
    /// Scan raw media and create a new corpus
    Scan,
    /// Index corpus to create searchable database
    Index,
    /// Process raw media files for transcoding
    Prepare,
}

// pub fn get_argsx() -> () {
//     clap::App::new(clap::crate_name!())
//         .version(clap::crate_version!())
//         .about(clap::crate_description!())
//         .setting(clap::AppSettings::DeriveDisplayOrder)
//         .arg(
//             clap::Arg::with_name("verbosity")
//                 .short("v")
//                 .multiple(true)
//                 .global(true)
//                 .help("Sets the level of verbosity"),
//         )
//         .subcommand(
//             SubCommand::with_name("media")
//                 .about("Commands to deal with processing and preparing media")
//                 .setting(clap::AppSettings::DeriveDisplayOrder)
//                 .arg(
//                     clap::Arg::with_name("index_name")
//                         .long("index-name")
//                         .global(true)
//                         .default_value("default")
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("storage")
//                         .long("storage")
//                         .global(true)
//                         .default_value(STORAGE_DEFAULT)
//                         .takes_value(true),
//                 )
//                 .subcommand(
//                     SubCommand::with_name("scan")
//                         .about("scan raw media and create a new corpus")
//                         .arg(
//                             clap::Arg::with_name("path")
//                                 .required(true)
//                                 .takes_value(true),
//                         ),
//                 )
//                 .subcommand(
//                     SubCommand::with_name("index")
//                         .about("index corpus to create a searchable database")
//                         .arg(
//                             clap::Arg::with_name("index_window")
//                                 .long("max-window")
//                                 .default_value(INDEX_WINDOW_DEFAULT)
//                                 .takes_value(true),
//                         ),
//                 )
//                 .subcommand(
//                     SubCommand::with_name("prepare")
//                         .about("process raw media files for transcoding"),
//                 ),
//         )
//         .subcommand(
//             SubCommand::with_name("index")
//                 .arg(
//                     clap::Arg::with_name("path")
//                         .long("path")
//                         .required(true)
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("storage")
//                         .long("storage")
//                         .default_value(STORAGE_DEFAULT)
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("index_window")
//                         .long("max-window")
//                         .default_value(INDEX_WINDOW_DEFAULT)
//                         .takes_value(true),
//                 ),
//         )
//         .subcommand(
//             SubCommand::with_name("search")
//                 .arg(
//                     clap::Arg::with_name("query")
//                         .long("query")
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("storage")
//                         .long("storage")
//                         .default_value(STORAGE_DEFAULT)
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("search_window")
//                         .long("search-window")
//                         .takes_value(true),
//                 ),
//         )
//         .subcommand(
//             SubCommand::with_name("transcode")
//                 .arg(clap::Arg::with_name("spec").multiple(true))
//                 .arg(
//                     clap::Arg::with_name("storage")
//                         .long("storage")
//                         .default_value(STORAGE_DEFAULT)
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("output_gif")
//                         .long("out")
//                         .short("o")
//                         .default_value(OUTPUT_DEFAULT)
//                         .takes_value(true),
//                 ),
//         )
//         .subcommand(SubCommand::with_name("demo"))
//         .subcommand(
//             SubCommand::with_name("scan-titles").arg(
//                 clap::Arg::with_name("path")
//                     .required(true)
//                     .takes_value(true),
//             ),
//         )
//         .subcommand(
//             SubCommand::with_name("interactive")
//                 .arg(
//                     clap::Arg::with_name("query")
//                         .long("query")
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("storage")
//                         .long("storage")
//                         .default_value(STORAGE_DEFAULT)
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("search_window")
//                         .long("search-window")
//                         .takes_value(true),
//                 )
//                 .arg(
//                     clap::Arg::with_name("output_gif")
//                         .long("out")
//                         .short("o")
//                         .default_value(OUTPUT_DEFAULT)
//                         .takes_value(true),
//                 ),
//         )
//         .get_matches();
// }

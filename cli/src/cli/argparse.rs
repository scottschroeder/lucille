use std::path::PathBuf;

use clap::Parser;

const STORAGE_DEFAULT: &str = "storage";
const INDEX_WINDOW_DEFAULT: &str = "5";
const OUTPUT_DEFAULT: &str = "out.gif";
const EXPORT_DEFAULT: &str = "out.json";

use app::DEFAULT_INDEX_WINDOW_SIZE;

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
    /// Commands working with the top-level Corpus
    #[clap(subcommand)]
    Corpus(CorpusCommand),

    /// Scan a directory for media & subtitles
    ScanChapters(ScanChaptersOpts),

    /// Index a set of subtitles to be searched
    Index(IndexCommand),

    /// Search an index
    Search(SearchCommand),

    /// Import data
    #[clap(subcommand)]
    Import(ImportCommand),

    /// Export data
    #[clap(subcommand)]
    Export(ExportCommand),

    /// Interactive Gif Creation
    Interactive(InteractiveOpts),

    /// Debugging Utilities
    #[clap(subcommand)]
    Debug(DebugCommand),
    // Process and prepare media
    // #[clap(subcommand)]
    // Media(MediaCommand),
    // Index(IndexCommand),

    // Library(LibraryCommand),
    // Transcode(PrepareCommand),
    // Test(PrepareCommand),
    // Render an image
    // #[clap(subcommand)]
    // Render(Render),
}

#[derive(Parser, Debug)]
pub struct SearchCommand {
    /// The search query
    pub query: Vec<String>,

    /// The UUID of the search index to use
    #[clap(long)]
    pub index: String,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,
}

#[derive(Parser, Debug)]
pub struct IndexCommand {
    pub corpus_name: String,

    #[clap(long, default_value_t=DEFAULT_INDEX_WINDOW_SIZE)]
    pub window_size: usize,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,
}

#[derive(Parser, Debug)]
pub struct PrepareCommand {}

#[derive(Parser, Debug)]
pub enum ExportCommand {
    /// Export all details for a corpus
    Corpus(ExportCorpusOpts),
}

#[derive(Parser, Debug)]
pub enum DebugCommand {
    /// Lookup all instances where a hash appears in the database
    HashLookup(HashLookup),

    /// Show the launch configuration/directories for the given settings.
    ShowConfig(ShowConfig),

    /// Split a media file into segments
    SplitMediaFile(SplitMediaFile),
}

#[derive(Parser, Debug)]
pub enum ImportCommand {
    /// Import all details for a corpus
    Corpus(ImportCorpusOpts),
}

#[derive(Parser, Debug)]
pub enum CorpusCommand {
    /// Create a new corpus
    New(CorpusNewOpts),
    /// List existing corpuses
    List(CorpusListOpts),
}

#[derive(Parser, Debug)]
pub struct ImportCorpusOpts {
    pub filename: std::path::PathBuf,

    #[clap(flatten)]
    pub db: DatabaseConfig,
}

#[derive(Parser, Debug)]
pub struct InteractiveOpts {
    /// The search query
    pub query: Vec<String>,

    /// The UUID of the search index to use
    #[clap(long)]
    pub index: Option<String>,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,
}

#[derive(Parser, Debug)]
pub struct HashLookup {
    /// Search the database for this hash
    pub hash: String,

    #[clap(flatten)]
    pub db: DatabaseConfig,
}

#[derive(Parser, Debug)]
pub struct ShowConfig {
    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,
}

#[derive(Parser, Debug)]
pub struct SplitMediaFile {
    /// The input media file
    pub input: PathBuf,

    /// The the output directory
    pub output: PathBuf,

    /// The split duration target (may not be exact)
    #[clap(long)]
    pub duration: f32,
}

#[derive(Parser, Debug)]
pub struct ExportCorpusOpts {
    /// Corpus to export
    pub corpus_name: String,

    /// File to export
    #[clap(long)]
    pub out: Option<std::path::PathBuf>,

    #[clap(flatten)]
    pub db: DatabaseConfig,
}

#[derive(Parser, Debug)]
pub struct ScanChaptersOpts {
    /// Root directory to start recursive scan
    pub dir: std::path::PathBuf,

    /// If a filepath is already known to our database, trust the hash instead of re-computing
    #[clap(long)]
    pub trust_known_hashes: bool,

    /// Attach these files to an existing corpus
    #[clap(long)]
    pub corpus_name: String,

    #[clap(flatten)]
    pub db: DatabaseConfig,
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
    /// If not provided, will attempt to read `DATABASE_URL` env var, then user dirs.
    #[clap(long)]
    pub database_path: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub struct StorageConfig {
    /// Path to search index directory
    ///
    /// If not provided, will use user dirs
    #[clap(long)]
    pub index_root: Option<std::path::PathBuf>,
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

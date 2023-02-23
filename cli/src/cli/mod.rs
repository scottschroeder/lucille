use clap::Parser;

use self::{
    export::{export_corpus, import_corpus},
    workflow::interactive_search,
};

mod cleanup {}
pub mod argparse;
mod cli_select;
mod corpus;
mod debug_utils;
mod export;
mod helpers;
mod prepare;
mod scan;
mod workflow;

#[derive(Parser, Debug)]
#[clap(version = clap::crate_version!(), author = "Scott S. <scottschroeder@sent.com>")]
pub struct CliOpts {
    #[clap(short, long, global = true, parse(from_occurrences))]
    pub verbose: u8,
    #[clap(subcommand)]
    pub subcmd: SubCommand,
}

pub fn get_args() -> CliOpts {
    CliOpts::parse()
}

#[derive(Parser, Debug)]
pub enum SubCommand {
    /// Commands working with the top-level Corpus
    #[clap(subcommand)]
    Corpus(argparse::CorpusCommand),

    /// Scan a directory for media & subtitles
    ScanChapters(argparse::ScanChaptersOpts),

    /// Index a set of subtitles to be searched
    Index(argparse::IndexCommand),

    /// Search an index
    Search(argparse::SearchCommand),

    /// Import data
    #[clap(subcommand)]
    Import(argparse::ImportCommand),

    /// Export data
    #[clap(subcommand)]
    Export(argparse::ExportCommand),

    /// Interactive Gif Creation
    Interactive(argparse::InteractiveOpts),

    /// Debugging Utilities
    #[clap(subcommand)]
    Debug(argparse::DebugCommand),
    // Process and prepare media
    // #[clap(subcommand)]
    // Media(MediaCommand),
    // Index(IndexCommand),

    // Library(LibraryCommand),
    /// Commands for pre-processing media
    #[clap(subcommand)]
    PrepareMedia(argparse::PrepareCommand),
    // Test(PrepareCommand),
    // Render an image
    // #[clap(subcommand)]
    // Render(Render),
}

impl CliOpts {
    pub async fn run(&self) -> anyhow::Result<()> {
        match &self.subcmd {
            SubCommand::Corpus(sub) => match sub {
                argparse::CorpusCommand::New(opts) => corpus::create_new_corpus(opts).await,
                argparse::CorpusCommand::List(opts) => corpus::list_all_corpus(opts).await,
            },
            SubCommand::ScanChapters(opts) => scan::scan_chapters(opts).await,
            SubCommand::Index(opts) => scan::index_subtitles(opts).await,
            SubCommand::Search(opts) => workflow::search(opts).await,
            SubCommand::Export(sub) => match sub {
                argparse::ExportCommand::Corpus(opts) => export_corpus(opts).await,
            },
            SubCommand::Import(sub) => match sub {
                argparse::ImportCommand::Corpus(opts) => import_corpus(opts).await,
            },
            SubCommand::Interactive(opts) => interactive_search(opts).await,
            SubCommand::Debug(sub) => match sub {
                argparse::DebugCommand::HashLookup(opts) => workflow::hash_lookup(opts).await,
                argparse::DebugCommand::ShowConfig(opts) => workflow::show_config(opts).await,
                argparse::DebugCommand::SplitMediaFile(opts) => {
                    debug_utils::split_media_file(opts).await
                }
                argparse::DebugCommand::DecryptMediaFile(opts) => {
                    debug_utils::decrypt_media_file(opts).await
                }
            },
            SubCommand::PrepareMedia(sub) => match sub {
                argparse::PrepareCommand::CreateMediaView(opts) => {
                    prepare::create_media_view(opts).await
                }
            },
        }
    }
}

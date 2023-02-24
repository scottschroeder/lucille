use clap::Parser;

pub mod argparse;
mod clean;
mod corpus;
mod debug_utils;
mod export;
mod helpers;
mod media_view;
mod scan;
mod search;

pub fn get_args() -> CliOpts {
    CliOpts::parse()
}

#[derive(Parser, Debug)]
#[clap(version = clap::crate_version!(), author = "Scott S. <scottschroeder@sent.com>")]
pub struct CliOpts {
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Parser, Debug)]
enum SubCommand {
    /// Commands working with the top-level Corpus
    #[clap(subcommand)]
    Corpus(corpus::CorpusCommand),

    /// Scan a directory for media & subtitles
    ScanChapters(scan::ScanChaptersOpts),

    /// Commands for working with media-views
    #[clap(subcommand)]
    MediaView(media_view::MediaViewCommand),

    /// Index a set of subtitles to be searched
    Index(scan::IndexCommand),

    /// Search an index
    Search(search::SearchCommand),

    /// Import data
    #[clap(subcommand)]
    Import(export::ImportCommand),

    /// Export data
    #[clap(subcommand)]
    Export(export::ExportCommand),

    /// Interactive Gif Creation
    Interactive(search::InteractiveOpts),

    /// Debugging Utilities
    #[clap(subcommand)]
    Debug(debug_utils::DebugCommand),

    /// Tools to cleanup and tidy the db or filesystem
    #[clap(subcommand)]
    Clean(clean::CleanCommand),

    Test(TestCommand),
}

impl CliOpts {
    pub async fn run(&self) -> anyhow::Result<()> {
        match &self.subcmd {
            SubCommand::Corpus(cmd) => cmd.run().await,
            SubCommand::ScanChapters(cmd) => cmd.run().await,
            SubCommand::Index(cmd) => cmd.run().await,
            SubCommand::Search(cmd) => cmd.run().await,
            SubCommand::Export(cmd) => cmd.run().await,
            SubCommand::Import(cmd) => cmd.run().await,
            SubCommand::Interactive(cmd) => cmd.run().await,
            SubCommand::Debug(cmd) => cmd.run().await,
            SubCommand::MediaView(cmd) => cmd.run().await,
            SubCommand::Clean(cmd) => cmd.run().await,
            SubCommand::Test(cmd) => do_test(cmd).await,
        }
    }
}

#[derive(Parser, Debug)]
pub struct TestCommand {
    #[clap(flatten)]
    pub cfg: argparse::AppConfig,
}

async fn do_test(args: &TestCommand) -> anyhow::Result<()> {
    let app = args.cfg.build_app().await?;
    Ok(())
}

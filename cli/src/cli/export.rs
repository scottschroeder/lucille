use anyhow::Context;
use clap::Parser;
use lucile_core::export::CorpusExport;

use super::argparse::DatabaseConfig;
use crate::cli::helpers;

#[derive(Parser, Debug)]
pub enum ExportCommand {
    /// Export all details for a corpus
    Corpus(ExportCorpusOpts),
}

impl ExportCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        match self {
            ExportCommand::Corpus(o) => export_corpus(o).await,
        }
    }
}

#[derive(Parser, Debug)]
pub enum ImportCommand {
    /// Import all details for a corpus
    Corpus(ImportCorpusOpts),
}

impl ImportCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        match self {
            ImportCommand::Corpus(o) => import_corpus(o).await,
        }
    }
}

#[derive(Parser, Debug)]
pub struct ImportCorpusOpts {
    pub filename: std::path::PathBuf,

    #[clap(flatten)]
    pub db: DatabaseConfig,
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

pub(crate) async fn import_corpus(args: &ImportCorpusOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    log::trace!("using app: {:?}", app);

    let packet: CorpusExport = {
        let f = std::fs::File::open(args.filename.as_path())
            .with_context(|| format!("could not import file: {:?}", args.filename))?;
        serde_json::from_reader(f).context("could not deserialize corpus export packet")?
    };

    app::import_corpus_packet(&app, &packet).await?;

    Ok(())
}

pub(crate) async fn export_corpus(args: &ExportCorpusOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    log::trace!("using app: {:?}", app);
    let corpus_id = app
        .db
        .get_corpus_id(&args.corpus_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("could not find corpus: {:?}", args.corpus_name))?;

    let packet = app::export_corpus_packet(&app, corpus_id).await?;

    if let Some(filename) = &args.out {
        let f = std::fs::File::create(filename.as_path())
            .with_context(|| format!("could not create file for output: {:?}", filename))?;
        serde_json::to_writer(f, &packet)?;
    } else {
        let out = serde_json::to_string_pretty(&packet)?;
        println!("{}", out);
    }

    Ok(())
}

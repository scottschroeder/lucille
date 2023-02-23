use clap::Parser;

use crate::cli::{argparse::DatabaseConfig, helpers};

#[derive(Parser, Debug)]
pub enum CorpusCommand {
    /// Create a new corpus
    New(CorpusNewOpts),
    /// List existing corpuses
    List(CorpusListOpts),
}

impl CorpusCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        match self {
            CorpusCommand::New(args) => create_new_corpus(args).await,
            CorpusCommand::List(args) => list_all_corpus(args).await,
        }
    }
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

pub(crate) async fn create_new_corpus(args: &CorpusNewOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    log::info!("creating new corpus with name: {:?}", args.name);
    let corpus = app.db.add_corpus(&args.name).await?;
    log::info!("inserted `{}` with id={}", args.name, corpus.id.unwrap());
    Ok(())
}

pub(crate) async fn list_all_corpus(args: &CorpusListOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    let corpus = app.db.list_corpus().await?;
    for c in corpus {
        println!("{:?}", c);
    }
    Ok(())
}

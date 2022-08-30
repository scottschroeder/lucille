pub mod argparse;
mod cli_select;
mod helpers;
mod media_intake;
mod corpus {
    use database::Database;

    use crate::cli::helpers;

    use super::argparse;

    pub(crate) async fn create_new_corpus(args: &argparse::CorpusNewOpts) -> anyhow::Result<()> {
        let db = helpers::get_database(&args.db).await?;
        log::info!("creating new corpus with name: {:?}", args.name);
        let corpus = db.add_corpus(&args.name).await?;
        log::info!("inserted `{}` with id={}", args.name, corpus.id.unwrap());
        Ok(())
    }

    pub(crate) async fn list_all_corpus(args: &argparse::CorpusListOpts) -> anyhow::Result<()> {
        let db = helpers::get_database(&args.db).await?;
        let corpus = db.list_corpus().await?;
        for c in corpus {
            println!("{:?}", c);
        }
        Ok(())
    }
}

mod scan {
    use app::{add_content_to_corpus, scan::scan_content};

    use crate::cli::helpers;

    use super::argparse;

    pub(crate) async fn scan_chapters(args: &argparse::ScanChaptersOpts) -> anyhow::Result<()> {
        let db = helpers::get_database(&args.db).await?;
        let corpus = db
            .get_or_add_corpus(args.corpus_name.as_str())
            .await?;
        log::debug!("using corpus: {:?}", corpus);

        let content = scan_content(args.dir.as_path())?;

        let x = add_content_to_corpus(&db, Some(&corpus), content).await?;
        Ok(())
    }
}

pub async fn run_cli(args: &argparse::CliOpts) -> anyhow::Result<()> {
    match &args.subcmd {
        argparse::SubCommand::Corpus(sub) => match sub {
            argparse::CorpusCommand::New(opts) => corpus::create_new_corpus(opts).await,
            argparse::CorpusCommand::List(opts) => corpus::list_all_corpus(opts).await,
        },
        argparse::SubCommand::ScanChapters(opts) => scan::scan_chapters(opts).await,
    }
}

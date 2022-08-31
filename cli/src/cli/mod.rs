pub mod argparse;
// mod cli_select;
mod helpers;
// mod media_intake;
mod corpus {

    use super::argparse;
    use crate::cli::helpers;

    pub(crate) async fn create_new_corpus(args: &argparse::CorpusNewOpts) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), None).await?;
        log::info!("creating new corpus with name: {:?}", args.name);
        let corpus = app.db.add_corpus(&args.name).await?;
        log::info!("inserted `{}` with id={}", args.name, corpus.id.unwrap());
        Ok(())
    }

    pub(crate) async fn list_all_corpus(args: &argparse::CorpusListOpts) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), None).await?;
        let corpus = app.db.list_corpus().await?;
        for c in corpus {
            println!("{:?}", c);
        }
        Ok(())
    }
}

mod scan {
    use app::{add_content_to_corpus, scan::scan_content};

    use super::argparse;
    use crate::cli::helpers;

    pub(crate) async fn scan_chapters(args: &argparse::ScanChaptersOpts) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), None).await?;
        let corpus = app.db.get_or_add_corpus(args.corpus_name.as_str()).await?;
        log::debug!("using corpus: {:?}", corpus);

        let content = scan_content(args.dir.as_path())?;

        add_content_to_corpus(&app.db, Some(&corpus), content).await?;
        Ok(())
    }
    pub(crate) async fn index_subtitles(args: &argparse::IndexCommand) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), Some(&args.storage)).await?;
        log::trace!("using app: {:?}", app);
        let corpus_id = app
            .db
            .get_corpus_id(&args.corpus_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("could not find corpus: {:?}", args.corpus_name))?;

        app::index_subtitles(&app, corpus_id, Some(args.window_size)).await?;

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
        argparse::SubCommand::Index(opts) => scan::index_subtitles(opts).await,
    }
}

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

mod workflow {
    use std::str::FromStr;

    use anyhow::Context;
    use app::search_manager::SearchRequest;
    use lucile_core::{clean_sub::CleanSubs, uuid::Uuid};

    use super::argparse;
    use crate::cli::helpers;

    const HIST: [&str; 6] = ["     ", "    *", "   **", "  ***", " ****", "*****"];
    pub(crate) async fn search(args: &argparse::SearchCommand) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), Some(&args.storage)).await?;
        log::trace!("using app: {:?}", app);

        let index_uuid = Uuid::from_str(&args.index).with_context(|| {
            format!(
                "provided search index `{}` is not a valid UUID",
                &args.index
            )
        })?;

        let searcher = app.search_service(index_uuid)?;
        let query = args.query.join(" ");
        log::info!("query: {:?}", query);
        let req = SearchRequest {
            query: &query,
            window: Some(5),
            max_responses: Some(3),
        };
        let resp = searcher
            .search_and_rank(req)
            .await
            .context("error doing search_and_rank")?;
        for clip in resp.results {
            let (_, m) = app.db.get_episode_by_id(clip.srt_id).await?;
            let subs = app.db.get_all_subs_for_srt(clip.srt_id).await?;
            println!("{:?}: {}", clip.score, m);

            let base = clip.offset;
            for (offset, linescore) in clip.lines.iter().enumerate() {
                let normalized = ((5.0 * linescore.score / clip.score) + 0.5) as usize;
                let script = CleanSubs(&subs[base + offset..base + offset + 1]);
                println!("  ({:2}) [{}]- {}", offset, HIST[normalized], script);
            }
            // println!("{:#?}", r);
        }

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
        argparse::SubCommand::Search(opts) => workflow::search(opts).await,
    }
}

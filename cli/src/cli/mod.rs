use self::{
    export::{export_corpus, import_corpus},
    workflow::interactive_search,
};

pub mod argparse;
mod cli_select;
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
    use app::{
        app::LucileApp,
        search_manager::{SearchRequest, SearchResponse},
        transcode::{MakeGifRequest, SubSegment, TranscodeRequest},
    };
    use lucile_core::{clean_sub::CleanSubs, metadata::MediaHash, uuid::Uuid};

    use super::argparse;
    use crate::cli::{cli_select, helpers};

    async fn setup_search(
        app: &LucileApp,
        index: Option<&str>,
        query: &str,
    ) -> anyhow::Result<SearchResponse> {
        log::trace!("using app: {:?}", app);

        let index_uuid = if let Some(index) = index {
            Uuid::from_str(index).with_context(|| {
                format!("provided search index `{}` is not a valid UUID", &index)
            })?
        } else {
            app.db
                .get_search_indexes()
                .await?
                .into_iter()
                .last()
                .ok_or_else(|| anyhow::anyhow!("unable to find recent search index"))?
        };

        let searcher = app.search_service(index_uuid)?;
        log::info!("query: {:?}", query);
        let req = SearchRequest {
            query,
            window: Some(5),
            max_responses: Some(3),
        };
        let resp = searcher
            .search_and_rank(req)
            .await
            .context("error doing search_and_rank")?;

        Ok(resp)
    }

    const HIST: [&str; 6] = ["     ", "    *", "   **", "  ***", " ****", "*****"];
    pub(crate) async fn search(args: &argparse::SearchCommand) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), Some(&args.storage)).await?;
        log::trace!("using app: {:?}", app);

        let query = args.query.join(" ");
        let resp = setup_search(&app, Some(args.index.as_str()), query.as_str()).await?;

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
        }

        Ok(())
    }

    pub(crate) async fn interactive_search(args: &argparse::InteractiveOpts) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), Some(&args.storage)).await?;
        log::trace!("using app: {:?}", app);

        let query = args.query.join(" ");
        let resp = setup_search(&app, args.index.as_deref(), query.as_str()).await?;
        let (clip, range) = cli_select::ask_user_for_clip(&app, &resp).await?;

        let sub_range = (clip.offset + range.start)..(clip.offset + range.end);

        let transcode_req = TranscodeRequest {
            request: app::transcode::RequestType::MakeGif(MakeGifRequest {
                segments: vec![SubSegment {
                    srt_id: clip.srt_id,
                    sub_range,
                }],
            }),
        };

        println!("{:#?}", transcode_req);

        Ok(())
    }

    pub(crate) async fn hash_lookup(args: &argparse::HashLookup) -> anyhow::Result<()> {
        let hash = MediaHash::from_str(&args.hash).context("could not parse hash")?;
        let app = helpers::get_app(Some(&args.db), None).await?;
        log::trace!("using app: {:?}", app);

        app::get_details_for_hash(&app, hash).await?;

        // println!("{:#?}", transcode_req);

        Ok(())
    }
}

mod export {
    use anyhow::Context;
    use lucile_core::export::CorpusExport;

    use super::argparse;
    use crate::cli::helpers;

    pub(crate) async fn import_corpus(args: &argparse::ImportCorpusOpts) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&args.db), None).await?;
        log::trace!("using app: {:?}", app);

        let packet: CorpusExport = {
            let f = std::fs::File::open(args.filename.as_path())
                .with_context(|| format!("could not import file: {:?}", args.filename))?;
            serde_json::from_reader(f).context("could not deserialize corpus export packet")?
        };

        app::import_corpus_packet(&app, packet).await?;

        Ok(())
    }

    pub(crate) async fn export_corpus(args: &argparse::ExportCorpusOpts) -> anyhow::Result<()> {
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
        argparse::SubCommand::Export(sub) => match sub {
            argparse::ExportCommand::Corpus(opts) => export_corpus(opts).await,
        },
        argparse::SubCommand::Import(sub) => match sub {
            argparse::ImportCommand::Corpus(opts) => import_corpus(opts).await,
        },
        argparse::SubCommand::Interactive(opts) => interactive_search(opts).await,
        argparse::SubCommand::HashLookup(opts) => workflow::hash_lookup(opts).await,
    }
}

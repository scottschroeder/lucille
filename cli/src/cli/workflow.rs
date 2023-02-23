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
        Uuid::from_str(index)
            .with_context(|| format!("provided search index `{}` is not a valid UUID", &index))?
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

pub(crate) async fn show_config(args: &argparse::ShowConfig) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), Some(&args.storage)).await?;
    println!("{:#?}", app);
    Ok(())
}

pub(crate) async fn hash_lookup(args: &argparse::HashLookup) -> anyhow::Result<()> {
    let hash = MediaHash::from_str(&args.hash).context("could not parse hash")?;
    let app = helpers::get_app(Some(&args.db), None).await?;
    log::trace!("using app: {:?}", app);

    app::print_details_for_hash(&app, hash).await?;
    Ok(())
}

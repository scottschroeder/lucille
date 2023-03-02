use std::str::FromStr;

use anyhow::Context;
use app::{
    app::LucilleApp,
    search_manager::{SearchRequest, SearchResponse},
    transcode::{MakeGifRequest, SubSegment},
};
use clap::Parser;
use lucille_core::{clean_sub::CleanSubs, uuid::Uuid};

mod select;

use super::argparse::{DatabaseConfig, StorageConfig};
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
pub struct InteractiveOpts {
    /// The search query
    pub query: Vec<String>,

    /// output gif file
    #[clap(long, default_value = "out.gif")]
    pub output: String,

    /// The UUID of the search index to use
    #[clap(long)]
    pub index: Option<String>,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,
}

impl InteractiveOpts {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let app = app::app::LucilleBuilder::new()?
            .database_path(self.db.database_path())?
            .index_root(self.storage.index_root())?
            .build()
            .await?;

        let query = self.query.join(" ");
        let resp = setup_search(&app, self.index.as_deref(), query.as_str()).await?;
        let (clip, range) = select::ask_user_for_clip(&app, &resp).await?;

        let sub_range = (clip.offset + range.start)..(clip.offset + range.end);
        let srt_uuid = app.db.get_srt_uuid_by_id(clip.srt_id).await?;

        let gif_request = MakeGifRequest {
            segments: vec![SubSegment {
                srt_uuid,
                sub_range,
            }],
        };

        let mut res = app::transcode::handle_make_gif_request(&app, &gif_request).await?;
        let mut output = res.output();
        let mut out_gif = tokio::fs::File::create(&self.output).await?;
        tokio::io::copy(&mut output, &mut out_gif).await?;
        res.wait().await?;
        Ok(())
    }
}

async fn setup_search(
    app: &LucilleApp,
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
impl SearchCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let app = app::app::LucilleBuilder::new()?
            .database_path(self.db.database_path())?
            .index_root(self.storage.index_root())?
            .build()
            .await?;

        let query = self.query.join(" ");
        let resp = setup_search(&app, Some(self.index.as_str()), query.as_str()).await?;

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
}

use std::str::FromStr;

use anyhow::Context;
use app::{
    app::LucileApp,
    ffmpeg::gif::{FFMpegGifTranscoder, GifSettings},
    search_manager::{SearchRequest, SearchResponse},
    transcode::{MakeGifRequest, SubSegment, TranscodeRequest},
};
use clap::Parser;
use lucile_core::{clean_sub::CleanSubs, uuid::Uuid};

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

    /// The UUID of the search index to use
    #[clap(long)]
    pub index: Option<String>,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,
}

pub async fn test_cmd(args: &super::TestCommand) -> anyhow::Result<()> {
    let app = args.cfg.build_app().await?;
    let srt_uuid = Uuid::from_str("1394603f-c591-400f-87f7-52c9c6f5be12").unwrap();
    let s = 278;
    let e = 279;

    let subs = app.db.get_all_subs_for_srt_by_uuid(srt_uuid).await?;
    let clip_subs = &subs[s..(e + 1)];

    let settings = GifSettings::default();
    let transcoder = FFMpegGifTranscoder::build_cmd(app.ffmpeg(), clip_subs, &settings).await?;
    log::debug!("{:#?}", transcoder);

    let views = app.db.get_media_views_for_srt(srt_uuid).await?;
    let orig = views.into_iter().find(|v| v.name == "original").unwrap();
    let segments = app.db.get_media_segment_by_view(orig.id).await?;
    let media = app
        .db
        .get_storage_by_hash(segments[0].hash)
        .await?
        .expect("could not find media");
    let input = tokio::fs::File::open(&media.path).await?;

    let (res, mut output) = transcoder.launch(Box::new(input)).await?;
    let mut out_gif = tokio::fs::File::open("out.gif").await?;
    log::debug!("copy stdout to out.gif");
    let b = tokio::io::copy(&mut output, &mut out_gif).await?;
    log::debug!("copy stdout to out.gif complete ({} bytes)", b);

    res.check().await?;

    Ok(())
}

impl InteractiveOpts {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let app = app::app::LucileBuilder::new()?
            .database_path(self.db.database_path())?
            .index_root(self.storage.index_root())?
            .build()
            .await?;

        let query = self.query.join(" ");
        let resp = setup_search(&app, self.index.as_deref(), query.as_str()).await?;
        let (clip, range) = select::ask_user_for_clip(&app, &resp).await?;

        let sub_range = (clip.offset + range.start)..(clip.offset + range.end);

        let srt_uuid = app.db.get_srt_uuid_by_id(clip.srt_id).await?;

        let subs = app.db.get_all_subs_for_srt_by_uuid(srt_uuid).await?;
        let clip_subs = &subs[sub_range.start..sub_range.end + 1];
        for s in clip_subs {
            println!("{}", s);
        }

        let transcode_req = TranscodeRequest {
            request: app::transcode::RequestType::MakeGif(MakeGifRequest {
                segments: vec![SubSegment {
                    srt_uuid,
                    sub_range,
                }],
            }),
        };

        let json_req = serde_json::to_string_pretty(&transcode_req)?;
        println!("{}", json_req);

        let settings = GifSettings::default();
        let transcoder = FFMpegGifTranscoder::build_cmd(app.ffmpeg(), clip_subs, &settings).await?;
        log::debug!("{:#?}", transcoder);

        let views = app.db.get_media_views_for_srt(srt_uuid).await?;
        let orig = views.into_iter().find(|v| v.name == "original").unwrap();
        let segments = app.db.get_media_segment_by_view(orig.id).await?;
        let media = app
            .db
            .get_storage_by_hash(segments[0].hash)
            .await?
            .expect("could not find media");
        let input = tokio::fs::File::open(&media.path).await?;

        let (res, mut output) = transcoder.launch(Box::new(input)).await?;
        let mut out_gif = tokio::fs::File::open("out.gif").await?;
        log::debug!("copy stdout to out.gif");
        let b = tokio::io::copy(&mut output, &mut out_gif).await?;
        log::debug!("copy stdout to out.gif complete ({} bytes)", b);

        res.check().await?;

        Ok(())
    }
}

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
impl SearchCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let app = app::app::LucileBuilder::new()?
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

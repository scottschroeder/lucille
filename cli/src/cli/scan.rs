use app::ingest;

use super::argparse;
use crate::cli::helpers;

pub(crate) async fn scan_chapters(args: &argparse::ScanChaptersOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    let corpus = app.db.get_or_add_corpus(args.corpus_name.as_str()).await?;
    log::debug!("using corpus: {:?}", corpus);

    let media_paths = ingest::scan_media_paths(args.dir.as_path())?;
    let processor = ingest::MediaProcessor {
        db: app.db.clone(), // TODO hide this clone (which should basically be an Arc lower down
        trust_hashes: args.trust_known_hashes,
    };
    let media = processor.process_all_media(&media_paths).await;
    ingest::add_content_to_corpus(&app.db, Some(&corpus), media).await?;
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

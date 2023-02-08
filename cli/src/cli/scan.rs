use super::argparse;
use crate::cli::helpers;

pub(crate) async fn scan_chapters(args: &argparse::ScanChaptersOpts) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;
    let corpus = app.db.get_or_add_corpus(args.corpus_name.as_str()).await?;
    log::debug!("using corpus: {:?}", corpus);

    app.media_scanner(args.trust_known_hashes)
        .ingest(args.dir.as_path(), Some(&corpus))
        .await?;
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

use super::{argparse, helpers};

pub(crate) async fn create_media_view(args: &argparse::CreateMediaView) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;

    let corpus_id = app
        .db
        .get_corpus_id(&args.corpus_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("could not find corpus: {:?}", args.corpus_name))?;

    let views = app.db.get_media_views_for_corpus(corpus_id).await?;

    // TODO allow for other modes like replace, or only do missing
    let mut conflict = false;
    for v in &views {
        if v.name == args.view_name {
            conflict = true;
            let chapter = app.db.get_chapter_by_id(v.chapter_id).await?;
            log::error!(
                "conflicting media view on id={} [{}]: {}",
                chapter.id,
                chapter.hash,
                chapter.metadata
            );
        }
    }

    if conflict {
        anyhow::bail!(
            "could not create view `{}` due to conflicts",
            args.view_name
        );
    }

    // Get all _active_ chapters for a corpus
    // Check access to all storage hashes
    // For each file
    //   do split
    //   create view
    //   add segments
    //   add storage

    todo!("create media view for corpus: {:?}", corpus_id)
}

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context;
use app::{
    ffmpeg::FFMpegBinary,
    prepare::{MediaProcessor, MediaSplittingStrategy},
    storage::FileCheckStrategy,
};
use database::Database;
use lucile_core::{export::ChapterExport, identifiers::CorpusId};

use super::{argparse, helpers};

pub(crate) async fn create_media_view(args: &argparse::CreateMediaView) -> anyhow::Result<()> {
    let app = helpers::get_app(Some(&args.db), None).await?;

    let corpus_id = app
        .db
        .get_corpus_id(&args.corpus_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("could not find corpus: {:?}", args.corpus_name))?;

    let chapters =
        check_filter_view_conflicts(&app.db, corpus_id, &args.view_name, args.skip_conflicts)
            .await?;
    // let views = app.db.get_media_views_for_corpus(corpus_id).await?;

    // // TODO allow for other modes like replace, or only do missing
    // let mut conflict = false;
    // for v in &views {
    //     if v.name == args.view_name {
    //         conflict = true;
    //         let chapter = app.db.get_chapter_by_id(v.chapter_id).await?;
    //         log::error!(
    //             "conflicting media view on id={} [{}]: {}",
    //             chapter.id,
    //             chapter.hash,
    //             chapter.metadata
    //         );
    //     }
    // }

    // if conflict {
    //     anyhow::bail!(
    //         "could not create view `{}` due to conflicts",
    //         args.view_name
    //     );
    // }

    // let all_chapters = app.db.get_active_chapters_for_corpus(corpus_id).await?;
    // let mut chapters = Vec::new();

    // let mut conflict = false;
    // for chapter in all_chapters {
    //     let chapter_conflict = app
    //         .db
    //         .get_media_views_for_chapter(chapter.id)
    //         .await
    //         .with_context(|| format!("getting media views for {:?}", chapter))?
    //         .iter()
    //         .any(|v| v.name == args.view_name);
    //     if chapter_conflict {
    //         if !args.skip_conflicts {
    //             log::error!(
    //                 "conflicting media view on id={} [{}]: {}",
    //                 chapter.id,
    //                 chapter.hash,
    //                 chapter.metadata
    //             );
    //         }
    //     } else {
    //         chapters.push(chapter);
    //     }
    //     conflict = conflict || chapter_conflict;
    // }
    if chapters.is_empty() {
        log::warn!("no chapters require processing");
        return Ok(());
    }

    log::info!("performing media split on {} chapters", chapters.len());

    let mut verify_source_set = tokio::task::JoinSet::new();
    for chapter in &chapters {
        let chapter = chapter.clone();
        let db = app.db.clone();
        let strategy = args.file_check_settings.check_strategy.to_app();
        verify_source_set.spawn(async move {
            check_storage_exists(&db, &chapter, strategy)
                .await
                .with_context(|| format!("unable to verify source for chapter: {:?}", chapter))
                .map(|p| (chapter.id, p))
        });
    }

    let mut pathmap = HashMap::new();
    let mut local_files_ok = true;
    while let Some(res) = verify_source_set.join_next().await {
        let res = res.context("task running storage check failed to join")?;
        match res {
            Ok((cid, p)) => {
                pathmap.insert(cid, p);
            }
            Err(e) => {
                log::error!("{:#}", e);
                local_files_ok = false;
            }
        }
    }

    if !local_files_ok {
        anyhow::bail!("could not prepare media due to missing source(s)");
    }

    let mut split_set = tokio::task::JoinSet::new();

    let output = args
        .media_storage
        .media_root
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("must define media root"))?;

    let ffmpeg: FFMpegBinary = args.ffmpeg.ffmpeg.clone().into();
    let split_buider = std::sync::Arc::new(
        app::prepare::MediaSplittingStrategy::new(
            ffmpeg,
            Duration::from_secs_f32(args.split_settings.duration),
            args.split_settings.encryption.to_app(),
            output,
        )
        .context("build split strategy")?,
    );

    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(args.parallel));
    for chapter in &chapters {
        let chapter = chapter.clone();
        let db = app.db.clone();
        let semaphore = semaphore.clone();
        let strategy = split_buider.clone();
        let path = pathmap[&chapter.id].clone();
        let view_name = args.view_name.clone();
        split_set.spawn(async move {
            let _permit = semaphore.acquire_owned().await.unwrap();
            do_split_on_chapter(&db, &view_name, &chapter, path.as_ref(), &strategy).await
        });
    }

    let mut all_ok = true;
    while let Some(res) = split_set.join_next().await {
        let ok = match res {
            Ok(Ok(())) => true,
            Ok(Err(e)) => {
                log::error!("failed at creating split: {}", e);
                false
            }

            Err(join_err) => {
                log::error!("task running split failed to join: {}", join_err);
                false
            }
        };
        all_ok = all_ok && ok;
    }

    if !all_ok {
        anyhow::bail!("failed to process all splits");
    }

    Ok(())
}

async fn check_filter_view_conflicts(
    db: &Database,
    corpus_id: CorpusId,
    view_name: &str,
    skip_conflicts: bool,
) -> anyhow::Result<Vec<ChapterExport>> {
    let all_chapters = db.get_active_chapters_for_corpus(corpus_id).await?;
    let mut chapters = Vec::new();

    let mut conflict = false;
    for chapter in all_chapters {
        let chapter_conflict = db
            .get_media_views_for_chapter(chapter.id)
            .await
            .with_context(|| format!("getting media views for {:?}", chapter))?
            .iter()
            .any(|v| v.name == view_name);
        if chapter_conflict {
            if !skip_conflicts {
                log::error!(
                    "conflicting media view on id={} [{}]: {}",
                    chapter.id,
                    chapter.hash,
                    chapter.metadata
                );
            }
        } else {
            chapters.push(chapter);
        }
        conflict = conflict || chapter_conflict;
    }

    if !skip_conflicts && conflict {
        anyhow::bail!("could not create view `{}` due to conflicts", view_name);
    }
    Ok(chapters)
}

async fn check_storage_exists(
    db: &Database,
    chapter: &ChapterExport,
    strategy: FileCheckStrategy,
) -> anyhow::Result<PathBuf> {
    log::debug!("verify storage for chapter: {:?}", chapter);
    let (path, outcome) = app::storage::check_local_file(db, chapter.hash, strategy)
        .await?
        .ok_or_else(|| anyhow::anyhow!("hash not found in database"))?;

    if !outcome.as_bool() {
        anyhow::bail!("failed validation: {:?}", outcome)
    }
    Ok(path)
}

async fn do_split_on_chapter<'a>(
    db: &Database,
    view_name: &str,
    chapter: &ChapterExport,
    path: &Path,
    strategy: &MediaSplittingStrategy,
) -> anyhow::Result<()> {
    let split = strategy.split_task(path);
    let processed_media = split
        .process()
        .await
        .context("error while splitting media")?;

    let media_view = db
        .add_media_view(chapter.id, view_name)
        .await
        .context("create media view name")?;

    for media in processed_media {
        let segment = db
            .add_media_segment(
                media_view.id,
                media.idx as u16,
                media.hash,
                media.start,
                media.key.clone(),
            )
            .await
            .context("add segment to db")?;
        db.add_storage(media.hash, &media.path)
            .await
            .context("add split to storage")?;

        log::debug!(
            "Successfully added segment={:?} chapter_id={:?} {:?}",
            segment,
            chapter.id,
            media
        );
    }
    Ok(())
}
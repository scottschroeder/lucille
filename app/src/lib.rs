use self::{app::LucileApp, scan::ScannedMedia};
use database::Database;
use lucile_core::{
    export::{CorpusExport, MediaExport, ViewOptions},
    identifiers::CorpusId,
    uuid::Uuid,
    ContentData, Corpus,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub mod app;
pub mod scan;
pub mod search_manager;
pub mod storage;
pub mod transcode;

pub const DEFAULT_INDEX_WINDOW_SIZE: usize = 5;

#[derive(Debug, thiserror::Error)]
pub enum LucileAppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] database::DatabaseError),
    #[error("failed to build search index")]
    BuildIndexError(#[from] search::error::TError),
    #[error("could not find video source")]
    MissingVideoSource,
}

pub async fn add_content_to_corpus(
    db: &Database,
    corpus: Option<&Corpus>,
    content: Vec<ScannedMedia>,
) -> Result<(), LucileAppError> {
    let corpus = corpus.expect("guess content name todo");

    let corpus_id = if let Some(id) = corpus.id {
        id
    } else {
        db.add_corpus(&corpus.title).await?.id.unwrap()
    };
    for file in &content {
        log::debug!("running file: {:?}", file);
    }

    // TODO do we care about bulk inserts?
    for file in &content {
        let (title, season, episode) = match &file.metadata {
            lucile_core::metadata::MediaMetadata::Episode(e) => (
                e.title.as_str(),
                Some(e.season as i64),
                Some(e.episode as i64),
            ),
            lucile_core::metadata::MediaMetadata::Unknown(u) => (u.as_str(), None, None),
        };
        let chapter_id = db
            .define_chapter(corpus_id, title, season, episode, file.hash)
            .await?;
        match &file.subs {
            scan::ScannedSubtitles::NotFound => {
                log::error!("not adding subtitles for {:?}: None Found", file);
            }
            scan::ScannedSubtitles::Error(e) => {
                log::error!("not adding subtitles for {:?}: {:?}", file, e);
            }
            scan::ScannedSubtitles::Subtitles(subs) => {
                let _uuid = db.add_subtitles(chapter_id, subs).await?;
            }
        }
        let media_view_id = db.add_media_view(chapter_id, "original").await?;
        db.add_media_segment(
            media_view_id,
            file.hash,
            Duration::default(),
            Duration::MAX,
            None,
        )
        .await?;
        db.add_storage(file.hash, &file.path).await?
    }

    Ok(())
}

pub async fn import_corpus_packet(
    app: &LucileApp,
    packet: CorpusExport,
) -> Result<CorpusId, LucileAppError> {
    let CorpusExport { title, content } = packet;

    let corpus = app.db.get_or_add_corpus(title).await?;
    let corpus_id = corpus.id.unwrap();

    for chapter in content {
        let MediaExport {
            views: ViewOptions { views },
            data:
                ContentData {
                    metadata,
                    hash,
                    local_id: _,
                    global_id: _,
                    subtitle,
                },
        } = chapter;

        let (title, season, episode) = match &metadata {
            lucile_core::metadata::MediaMetadata::Episode(e) => (
                e.title.as_str(),
                Some(e.season as i64),
                Some(e.episode as i64),
            ),
            lucile_core::metadata::MediaMetadata::Unknown(u) => (u.as_str(), None, None),
        };
        let chapter_id = app
            .db
            .define_chapter(corpus_id, title, season, episode, hash)
            .await?;
        app.db.add_subtitles(chapter_id, &subtitle).await?;
        for name in views {
            app.db.add_media_view(chapter_id, name).await?;
        }
    }

    Ok(corpus_id)
}

pub async fn export_corpus_packet(
    app: &LucileApp,
    corpus_id: CorpusId,
) -> Result<CorpusExport, LucileAppError> {
    let title = app.db.get_corpus(corpus_id).await?.title;
    let (_, content) = app.db.get_all_subs_for_corpus(corpus_id).await?;
    //
    let mut export = Vec::with_capacity(content.len());

    for c in content {
        let views = app
            .db
            .get_srt_view_options(c.global_id)
            .await?
            .into_iter()
            .map(|(_id, name)| name)
            .collect();
        export.push(MediaExport {
            views: ViewOptions { views },
            data: c,
        });
    }

    Ok(CorpusExport {
        title,
        content: export,
    })
}

pub async fn index_subtitles(
    app: &LucileApp,
    corpus_id: CorpusId,
    max_window: Option<usize>,
) -> Result<(), LucileAppError> {
    log::info!("performing index for {}", corpus_id);

    let (srts, all_subs) = app.db.get_all_subs_for_corpus(corpus_id).await?;

    log::trace!("ALL SUBS: {:#?}", all_subs);

    let index_uuid = Uuid::generate();
    let index_path = app.index_root().join(index_uuid.to_string());
    std::fs::create_dir_all(&index_path)?;
    let index = search::build_index(
        index_uuid,
        &index_path,
        all_subs.into_iter(),
        max_window.unwrap_or(DEFAULT_INDEX_WINDOW_SIZE),
    )?;

    log::info!("created index: {:?}", index);

    app.db.assoc_index_with_srts(index_uuid, srts).await?;

    Ok(())
}

// pub fn guess_content_name(content: &[ScannedMedia]) -> String {
//     let mut content_name_guesser = HashMap::new();
//     for (path, e) in media {
//         let (metadata, name_guess) = extract_metadata(e.title.as_str());
//         if let Some(name) = name_guess {
//             *content_name_guesser.entry(name).or_insert(0) += 1;
//         }
//         let content_data = ContentData {
//             subtitle: e.subtitles,
//             media_hash: e.media_hash,
//             metadata,
//         };
//         media_file_map.insert(e.media_hash, path);
//         content.push(content_data);
//     }
//     let content_name = content_name_guesser
//         .into_iter()
//         .max_by_key(|(_, v)| *v)
//         .map(|(s, _)| s);

//     ContentScanResults {
//         suggested_name: content_name,
//         files: media_file_map,
//         content,
//     }
// }

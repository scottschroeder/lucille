use std::time::Duration;

use database::Database;
use lucile_core::{uuid::Uuid, Corpus, CorpusId};

use self::{app::LucileApp, scan::ScannedMedia};

pub mod app;
pub mod scan;
pub mod search_manager;

pub const DEFAULT_INDEX_WINDOW_SIZE: usize = 5;

#[derive(Debug, thiserror::Error)]
pub enum LucileAppError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Database(#[from] database::DatabaseError),
    #[error("failed to build search index")]
    BuildIndexError(#[from] search::error::TError),
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
            scan::ScannedSubtitles::Subtitles(subs) => db.add_subtitles(chapter_id, subs).await?,
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

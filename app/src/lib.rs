


use lucile_core::{
    export::{CorpusExport, MediaExport, ViewOptions},
    identifiers::CorpusId,
    metadata::MediaHash,
    uuid::Uuid,
    ContentData,
};

use self::{
    app::LucileApp,
};

pub mod app;
pub mod ingest;
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

pub async fn print_details_for_hash(
    app: &LucileApp,
    hash: MediaHash,
) -> Result<(), LucileAppError> {
    // Lookup chapter with hash
    if let Some(chapter) = app.db.get_chapter_by_hash(hash).await? {
        let corpus = app.db.get_corpus(chapter.corpus_id).await?;
        println!("{}: {:#?}", corpus.title, chapter);
    }

    // Lookup segments with hash
    let segment = app.db.get_media_segment_by_hash(hash).await?;
    if let Some(s) = segment {
        let media_view = app.db.get_media_view(s.media_view_id).await?;
        println!("found view {:#?}\nsegment: {:#?}", media_view, s);
    }

    // Lookup storage with hash
    if let Some(storage) = app.db.get_storage_by_hash(hash).await? {
        println!("found media storage {:#?}", storage);
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
            .get_media_views_for_srt(c.global_id)
            .await?
            .into_iter()
            .map(|view| view.name)
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

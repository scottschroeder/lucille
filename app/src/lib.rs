use std::time::Duration;

use database::Database;
use lucile_core::Corpus;

use self::scan::ScannedMedia;
pub mod scan;

#[derive(Debug, thiserror::Error)]
pub enum LucileAppError {
    #[error(transparent)]
    Database(#[from] database::DatabaseError),
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
        //     let media_view_id = db.add_media_view(chapter_id, "original").await?;
        //     db.add_media_segment(media_view_id, file.hash, 0, Duration::MAX)
        //         .await?;
        //     db.add_storage(file.hash, file.path).await?
    }

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

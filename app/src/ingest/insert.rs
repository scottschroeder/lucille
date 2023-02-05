use std::time::Duration;

use database::Database;
use lucile_core::{
    identifiers::{ChapterId, CorpusId},
    Corpus,
};

use super::{ScannedMedia, ScannedSubtitles};
use crate::LucileAppError;

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

    for media in &content {
        add_scanned_media_to_db(db, corpus_id, media).await?;
    }

    Ok(())
}

pub(crate) async fn add_scanned_media_to_db(
    db: &Database,
    corpus_id: CorpusId,
    media: &ScannedMedia,
) -> Result<ChapterId, LucileAppError> {
    log::trace!("insert media into db: {:?}", media);
    let (title, season, episode) = match &media.metadata {
        lucile_core::metadata::MediaMetadata::Episode(e) => (
            e.title.as_str(),
            Some(e.season as i64),
            Some(e.episode as i64),
        ),
        lucile_core::metadata::MediaMetadata::Unknown(u) => (u.as_str(), None, None),
    };
    let chapter_id = db
        .define_chapter(corpus_id, title, season, episode, media.hash)
        .await?;
    match &media.subs {
        ScannedSubtitles::NotFound => {
            log::error!("not adding subtitles for {:?}: None Found", media);
        }
        ScannedSubtitles::Error(e) => {
            log::error!("not adding subtitles for {:?}: {:?}", media, e);
        }
        ScannedSubtitles::Subtitles(subs) => {
            let _uuid = db.add_subtitles(chapter_id, subs).await?;
        }
    }
    let media_view_id = db.add_media_view(chapter_id, "original").await?;
    db.add_media_segment(media_view_id.id, 0, media.hash, Duration::default(), None)
        .await?;
    db.add_storage(media.hash, &media.path).await?;
    Ok(chapter_id)
}

#[cfg(test)]
mod tests {
    use lucile_core::{
        metadata::{EpisodeMetadata, MediaHash},
        test_util::generate_subtitle,
    };

    use super::*;
    use crate::app::tests::lucile_test_app;

    #[tokio::test]
    async fn add_media_to_db() {
        let tapp = lucile_test_app().await;
        let corpus = tapp.app.db.add_corpus("show name").await.unwrap();

        let fname = std::path::PathBuf::from("/path/to/file");
        let subs = generate_subtitle(&["line1"]);
        let hash = MediaHash::from_bytes(b"data");
        let metadata = lucile_core::metadata::MediaMetadata::Episode(EpisodeMetadata {
            season: 3,
            episode: 12,
            title: "ep title".to_owned(),
        });

        let chapter_id = add_scanned_media_to_db(
            &tapp.app.db,
            corpus.id.unwrap(),
            &ScannedMedia {
                path: fname.clone(),
                subs: ScannedSubtitles::Subtitles(subs),
                hash,
                metadata: metadata.clone(),
            },
        )
        .await
        .expect("failure adding show to db");

        let storage = tapp
            .app
            .db
            .get_storage_by_hash(hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(storage.path, fname);
        assert_eq!(storage.hash, hash);

        let view_opts = tapp
            .app
            .db
            .get_media_view_options(chapter_id)
            .await
            .unwrap();
        assert_eq!(view_opts[0].1, "original");

        let chapter = tapp
            .app
            .db
            .get_chapter_by_hash(hash)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(chapter.metadata, metadata);

        let media_segment = tapp
            .app
            .db
            .get_media_segment_by_hash(hash)
            .await
            .unwrap()
            .unwrap();
        assert!(media_segment.key.is_none());
        assert_eq!(media_segment.start, Duration::default());
    }

    #[tokio::test]
    async fn add_same_media_twice() {
        let tapp = lucile_test_app().await;
        let corpus = tapp.app.db.add_corpus("show name").await.unwrap();

        let fname = std::path::PathBuf::from("/path/to/file");
        let subs = generate_subtitle(&["line1"]);
        let hash = MediaHash::from_bytes(b"data");
        let metadata = lucile_core::metadata::MediaMetadata::Episode(EpisodeMetadata {
            season: 3,
            episode: 12,
            title: "ep title".to_owned(),
        });

        let _c1 = add_scanned_media_to_db(
            &tapp.app.db,
            corpus.id.unwrap(),
            &ScannedMedia {
                path: fname.clone(),
                subs: ScannedSubtitles::Subtitles(subs.clone()),
                hash,
                metadata: metadata.clone(),
            },
        )
        .await
        .expect("failure adding show to db");

        // TODO this should work!
        let _c2 = add_scanned_media_to_db(
            &tapp.app.db,
            corpus.id.unwrap(),
            &ScannedMedia {
                path: fname.clone(),
                subs: ScannedSubtitles::Subtitles(subs),
                hash,
                metadata: metadata.clone(),
            },
        )
        .await;
        // .expect("failure adding duplicate show to db");
    }
}

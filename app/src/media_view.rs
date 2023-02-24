use database::Database;
use lucile_core::{export::ChapterExport, identifiers::CorpusId, media_segment::MediaView};

use crate::LucileAppError;

pub async fn get_media_view_in_corpus(
    db: &Database,
    corpus_id: CorpusId,
    view_name: &str,
) -> Result<Vec<(ChapterExport, Option<MediaView>)>, LucileAppError> {
    let all_chapters = db.get_active_chapters_for_corpus(corpus_id).await?;
    let mut results = Vec::with_capacity(all_chapters.len());

    for chapter in all_chapters {
        let mediaview = db
            .get_media_views_for_chapter(chapter.id)
            .await?
            .into_iter()
            .find(|v| v.name == view_name);
        results.push((chapter, mediaview))
    }

    Ok(results)
}

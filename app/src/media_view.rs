use std::time::Duration;

use database::Database;
use lucille_core::{
    export::ChapterExport,
    identifiers::{CorpusId, MediaViewId},
    media_segment::{MediaSegment, MediaView},
    uuid::Uuid,
};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::app::LucilleApp;

pub async fn get_media_view_for_transcode(
    app: &LucilleApp,
    srt_uuid: Uuid,
) -> anyhow::Result<Option<MediaView>> {
    let views = app.db.get_media_views_for_srt(srt_uuid).await?;
    let priorities = app.config.media_view_priority();
    Ok(select_best_media_view(&priorities, views))
}

// TODO do we need to see if we actually have segments around? what about remote?
fn select_best_media_view(priorities: &[String], mut views: Vec<MediaView>) -> Option<MediaView> {
    for p in priorities {
        for idx in 0..views.len() {
            if views[idx].name == *p {
                return Some(views.swap_remove(idx));
            }
        }
    }
    views.into_iter().next()
}

pub async fn get_media_view_in_corpus(
    db: &Database,
    corpus_id: CorpusId,
    view_name: &str,
) -> anyhow::Result<Vec<(ChapterExport, Option<MediaView>)>> {
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

pub async fn get_surrounding_media(
    app: &LucilleApp,
    media_view_id: MediaViewId,
    start: Duration,
    end: Duration,
) -> anyhow::Result<(Duration, Box<dyn AsyncRead + Unpin + Send>)> {
    // find segments that match our window
    let segments = app.db.get_media_segment_by_view(media_view_id).await?;
    let target_segments = cut_relevant_segments(&segments, start, end);

    let mut chain: Box<dyn AsyncRead + Unpin + Send> = Box::new(tokio::io::empty());
    let start = target_segments.first().map(|s| s.start).unwrap_or_default();
    for t in target_segments {
        log::trace!("getting media for {:?}", t);
        let rdr = crate::storage::backend::get_reader_for_segment(app, t).await?;
        chain = Box::new(chain.chain(rdr));
    }
    Ok((start, chain))
}

fn cut_indicies(segments: &[MediaSegment], start: Duration, end: Duration) -> (usize, usize) {
    let mut sidx = 0;

    for (idx, s) in segments.iter().enumerate() {
        if s.start < start {
            sidx = idx;
        } else if s.start > end {
            return (sidx, idx);
        }
    }

    (sidx, segments.len())
}

fn cut_relevant_segments(
    segments: &[MediaSegment],
    start: Duration,
    end: Duration,
) -> &[MediaSegment] {
    let (sidx, eidx) = cut_indicies(segments, start, end);
    &segments[sidx..eidx]
}

#[cfg(test)]
mod test {
    use lucille_core::MediaHash;

    use super::*;

    fn prepare_segments() -> Vec<MediaSegment> {
        (0..6)
            .map(|idx| MediaSegment {
                id: lucille_core::identifiers::MediaSegmentId::new(idx + 1),
                media_view_id: lucille_core::identifiers::MediaViewId::new(1),
                hash: MediaHash::from_bytes(format!("data_{}", idx).as_bytes()),
                start: Duration::from_secs_f32(idx as f32 * 10.0),
                key: None,
            })
            .collect()
    }

    /*
    0         10        20        30        40        50
    |---------|---------|---------|---------|---------|---------
    |    0    |    1    |    2    |    3    |    4    |    5
    */

    fn run_test_cut(start: u64, end: u64, sidx: usize, len: usize) {
        let segments = prepare_segments();
        let dstart = Duration::from_secs(start);
        let dend = Duration::from_secs(end);
        let a_cuts = cut_indicies(&segments, dstart, dend);

        // assert_eq!(a_cuts.0, sidx);
        assert_eq!(a_cuts, (sidx, sidx + len));
    }

    #[test]
    fn cut_first_lower_border() {
        run_test_cut(0, 2, 0, 1);
    }
    #[test]
    fn cut_first_on_boundary() {
        run_test_cut(0, 10, 0, 2);
    }
    #[test]
    fn cut_first_next_segment() {
        run_test_cut(0, 11, 0, 2);
    }

    #[test]
    fn cut_first_inside() {
        run_test_cut(1, 2, 0, 1);
    }

    #[test]
    fn cut_first_upper_border() {
        run_test_cut(1, 10, 0, 2);
    }

    #[test]
    fn cut_first_cross_second() {
        run_test_cut(1, 11, 0, 2);
    }

    #[test]
    fn cut_inside_middle() {
        run_test_cut(24, 25, 2, 1);
    }

    #[test]
    fn cut_cross_two() {
        run_test_cut(24, 35, 2, 2);
    }

    #[test]
    fn cut_cross_three() {
        run_test_cut(24, 45, 2, 3);
    }

    #[test]
    fn cut_end_after_end() {
        run_test_cut(44, 70, 4, 2);
    }

    #[test]
    fn cut_entierly_last_segment() {
        run_test_cut(55, 70, 5, 1);
    }
}

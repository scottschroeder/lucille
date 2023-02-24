use lucile_core::{
    identifiers::{ChapterId, CorpusId, MediaViewId},
    media_segment::MediaView,
    uuid::Uuid,
};

use crate::{Database, DatabaseError};

impl Database {
    pub async fn add_media_view<S: Into<String>>(
        &self,
        chapter_id: ChapterId,
        name: S,
    ) -> Result<MediaView, DatabaseError> {
        let name = name.into();

        let cid = chapter_id.get();
        let id = sqlx::query!(
            r#"
                    INSERT INTO media_view (chapter_id, name)
                    VALUES ( ?1, ?2 )
                    "#,
            cid,
            name,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(MediaView {
            id: MediaViewId::new(id),
            chapter_id,
            name,
        })
    }

    pub async fn get_media_view(
        &self,
        media_view_id: MediaViewId,
    ) -> Result<MediaView, DatabaseError> {
        let id = media_view_id.get();
        let row = sqlx::query!(
            r#"
                    SELECT
                        id, chapter_id, name
                    FROM media_view
                    WHERE
                        id = ?
                    "#,
            id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(MediaView {
            id: MediaViewId::new(row.id),
            chapter_id: ChapterId::new(row.chapter_id),
            name: row.name,
        })
    }

    pub async fn lookup_media_view(
        &self,
        chapter_id: ChapterId,
        name: &str,
    ) -> Result<Option<MediaView>, DatabaseError> {
        let id = chapter_id.get();
        let row_opt = sqlx::query!(
            r#"
                    SELECT
                        id, chapter_id, name
                    FROM media_view
                    WHERE
                        chapter_id = ?
                        AND name = ?
                    "#,
            id,
            name,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row_opt.map(|row| MediaView {
            id: MediaViewId::new(row.id),
            chapter_id: ChapterId::new(row.chapter_id),
            name: row.name,
        }))
    }

    pub async fn get_media_views_for_chapter(
        &self,
        chapter_id: ChapterId,
    ) -> Result<Vec<MediaView>, DatabaseError> {
        let cid = chapter_id.get();
        let rows = sqlx::query!(
            r#"
                SELECT 
                    id, name
                FROM media_view
                WHERE
                    chapter_id = ?
                ORDER BY
                    id ASC
         "#,
            cid
        )
        .map(|row| MediaView {
            id: MediaViewId::new(row.id),
            chapter_id,
            name: row.name,
        })
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_media_views_for_srt(
        &self,
        srt_uuid: Uuid,
    ) -> Result<Vec<MediaView>, DatabaseError> {
        let uuid = srt_uuid.to_string();
        let rows = sqlx::query!(
            r#"
                SELECT 
                    media_view.id, media_view.chapter_id, media_view.name
                FROM media_view
                JOIN srtfile
                  ON srtfile.chapter_id = media_view.chapter_id
                WHERE
                    srtfile.uuid = ?
                ORDER BY
                    media_view.id DESC
         "#,
            uuid
        )
        .map(|r| MediaView {
            id: MediaViewId::new(r.id),
            chapter_id: ChapterId::new(r.chapter_id),
            name: r.name,
        })
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Fetch all views for a corpus
    pub async fn get_media_views_for_corpus(
        &self,
        corpus_id: CorpusId,
    ) -> Result<Vec<MediaView>, DatabaseError> {
        let cid = corpus_id.get();
        let rows = sqlx::query!(
            r#"
                SELECT 
                    media_view.id, media_view.chapter_id, media_view.name
                FROM media_view
                JOIN chapter
                  ON chapter.id = media_view.chapter_id
                WHERE
                    chapter.corpus_id = ?
                ORDER BY
                    media_view.id DESC
         "#,
            cid,
        )
        .map(|r| MediaView {
            id: MediaViewId::new(r.id),
            chapter_id: ChapterId::new(r.chapter_id),
            name: r.name,
        })
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete a media_view by Id, along with all of the associated media_segments.
    /// Does not delete storage paths and does not delete any files,
    /// this is purely a db operation.
    pub async fn delete_media_view(&self, media_view_id: MediaViewId) -> Result<(), DatabaseError> {
        let id = media_view_id.get();

        sqlx::query!(
            r#"
            DELETE FROM media_view
            WHERE id = ?
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Rename a media-view for an entire corpus. This will update every chapter.
    /// This operation will fail (safely) if the `dst` name is already in use within
    /// the corpus.
    pub async fn rename_media_view(
        &self,
        corpus_id: CorpusId,
        src: &str,
        dst: &str,
    ) -> Result<(), DatabaseError> {
        let cid = corpus_id.get();

        sqlx::query!(
            r#"
            UPDATE media_view
            SET name = ?
            WHERE EXISTS (
                SELECT media_view.name
                FROM media_view
                JOIN chapter ON media_view.chapter_id = chapter.id
                JOIN corpus ON chapter.corpus_id = corpus.id
                WHERE corpus.id = ?
                    AND media_view.name = ?
            )
            "#,
            dst,
            cid,
            src
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use std::time::Duration;

    use lucile_core::{identifiers::MediaViewId, metadata::MediaHash};

    use super::*;
    use crate::database_test::assert_err_is_constraint;

    async fn add_two_segments(db: &Database, media_view_id: MediaViewId) -> (MediaHash, MediaHash) {
        let mk_hash = |idx: usize| {
            MediaHash::from_bytes(
                format!("view_{}_segment_{}", media_view_id.get(), idx).as_bytes(),
            )
        };
        let h0 = mk_hash(0);
        db.add_media_segment(media_view_id, 0, h0, Duration::from_secs_f64(0.0), None)
            .await
            .unwrap();

        let h1 = mk_hash(1);
        db.add_media_segment(media_view_id, 1, h1, Duration::from_secs_f64(10.0), None)
            .await
            .unwrap();
        (h0, h1)
    }

    #[tokio::test]
    async fn define_media_view() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        let view = db.add_media_view(ch_id, "test-view").await.unwrap();
        assert_eq!(view.id, MediaViewId::new(1))
    }

    #[tokio::test]
    async fn define_and_get_media_view() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        let view_insert = db.add_media_view(ch_id, "test-view").await.unwrap();
        db.add_media_view(ch_id, "extra").await.unwrap();
        let view_get = db.get_media_view(view_insert.id).await.unwrap();
        assert_eq!(view_get, view_insert)
    }

    #[tokio::test]
    async fn define_nameless_media_view() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        assert_err_is_constraint(db.add_media_view(ch_id, "").await, "CHECK");
    }

    #[tokio::test]
    async fn define_and_lookup_media_view() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        let view_insert = db.add_media_view(ch_id, "test-view").await.unwrap();
        let view_get = db.lookup_media_view(ch_id, "test-view").await.unwrap();
        assert_eq!(view_get, Some(view_insert))
    }

    #[tokio::test]
    async fn define_and_lookup_non_existant_media_view() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        db.add_media_view(ch_id, "test-view").await.unwrap();
        let view_get = db.lookup_media_view(ch_id, "not-real-view").await.unwrap();
        assert_eq!(view_get, None)
    }

    #[tokio::test]
    async fn delete_an_empty_media_view() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        let view = db.add_media_view(ch_id, "test-view").await.unwrap();
        let view2 = db.add_media_view(ch_id, "test-other-view").await.unwrap();
        db.delete_media_view(view.id).await.unwrap();
        let res = db.get_media_view(view.id).await;
        match res {
            Err(DatabaseError::Sqlx(sqlx::Error::RowNotFound)) => {}
            _ => panic!("incorrect error: {:?}", res),
        }
        db.get_media_view(view2.id).await.unwrap();
    }

    #[tokio::test]
    async fn delete_an_media_view_and_segments() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();

        let view = db.add_media_view(ch_id, "test-view").await.unwrap();
        let (v0s0, v0s1) = add_two_segments(&db, view.id).await;

        let view2 = db.add_media_view(ch_id, "test-other-view").await.unwrap();
        let (v1s0, v1s1) = add_two_segments(&db, view2.id).await;

        db.delete_media_view(view.id).await.unwrap();

        assert!(db.get_media_segment_by_hash(v0s0).await.unwrap().is_none());
        assert!(db.get_media_segment_by_hash(v0s1).await.unwrap().is_none());
        assert!(db.get_media_segment_by_hash(v1s0).await.unwrap().is_some());
        assert!(db.get_media_segment_by_hash(v1s1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn rename_media_view_simple() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();

        let view = db.add_media_view(ch_id, "test-view").await.unwrap();
        db.rename_media_view(corpus.id.unwrap(), "test-view", "new-name")
            .await
            .unwrap();
        let view_v2 = db.get_media_view(view.id).await.unwrap();
        assert_eq!(view.id, view_v2.id);
        assert_eq!(view_v2.name, "new-name");
    }

    #[tokio::test]
    async fn rename_media_view_with_conflict() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch1 = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();

        let ch2 = db
            .define_chapter(
                corpus.id.unwrap(),
                "c2",
                None,
                None,
                MediaHash::from_bytes(b"data2"),
            )
            .await
            .unwrap();

        let ch1_view1 = db.add_media_view(ch1, "view1").await.unwrap();
        let ch2_view1 = db.add_media_view(ch2, "view1").await.unwrap();
        let ch2_view2 = db.add_media_view(ch2, "view2").await.unwrap();

        let res = db
            .rename_media_view(corpus.id.unwrap(), "view1", "view2")
            .await;
        assert_err_is_constraint(res, "UNIQUE");

        let ch1_views = db.get_media_views_for_chapter(ch1).await.unwrap();
        let ch2_views = db.get_media_views_for_chapter(ch2).await.unwrap();

        assert_eq!(
            ch1_views,
            vec![MediaView {
                id: ch1_view1.id,
                chapter_id: ch1,
                name: "view1".to_string(),
            }]
        );

        assert_eq!(
            ch2_views,
            vec![
                MediaView {
                    id: ch2_view1.id,
                    chapter_id: ch2,
                    name: "view1".to_string(),
                },
                MediaView {
                    id: ch2_view2.id,
                    chapter_id: ch2,
                    name: "view2".to_string(),
                },
            ]
        );
    }
}

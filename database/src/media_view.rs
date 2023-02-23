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
                    id DESC
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
}

#[cfg(test)]
mod test {

    use lucile_core::{identifiers::MediaViewId, metadata::MediaHash};

    use super::*;
    use crate::database_test::assert_err_is_constraint;

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
}

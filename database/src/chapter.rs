use lucille_core::{
    export::ChapterExport,
    identifiers::{ChapterId, CorpusId},
    metadata::MediaHash,
};

use crate::{metadata_from_chapter, parse_media_hash, Database, DatabaseError};

impl Database {
    pub async fn define_chapter<S: Into<String>>(
        &self,
        corpus_id: CorpusId,
        title: S,
        season: Option<i64>,
        episode: Option<i64>,
        hash: MediaHash,
    ) -> Result<ChapterId, DatabaseError> {
        let title = title.into();
        log::trace!(
            "define chapter: C={}, title={:?}, S[{:?}] E[{:?}] {:?}",
            corpus_id,
            title,
            season,
            episode,
            hash
        );

        let cid = corpus_id.get();
        let hash_data = hash.to_string();
        let c = sqlx::query!(
            r#"
                UPDATE chapter
                SET
                    corpus_id = ?1,
                    title = ?2,
                    season = ?3,
                    episode = ?4
                WHERE
                    hash = ?5
            "#,
            cid,
            title,
            season,
            episode,
            hash_data
        )
        .execute(&self.pool)
        .await?;
        c.rows_affected();

        log::trace!("UPDATE RESULT: {:?}", c);

        let id = if c.rows_affected() > 0 {
            sqlx::query!(
                r#"
                    SELECT
                        id
                    FROM chapter
                    WHERE
                        hash = ?
                "#,
                hash_data
            )
            .fetch_one(&self.pool)
            .await?
            .id
        } else {
            sqlx::query!(
                r#"
                    INSERT INTO chapter (corpus_id, title, season, episode, hash)
                    VALUES ( ?1, ?2, ?3, ?4, ?5 )
                    "#,
                cid,
                title,
                season,
                episode,
                hash_data
            )
            .execute(&self.pool)
            .await?
            .last_insert_rowid()
        };
        log::trace!("UPDATE ID: {:?}", id);
        Ok(ChapterId::new(id))
    }

    pub async fn get_chapter_by_hash(
        &self,
        hash: MediaHash,
    ) -> Result<Option<ChapterExport>, DatabaseError> {
        let hash_data = hash.to_string();
        let row_opt = sqlx::query!(
            r#"
                    SELECT
                        id, corpus_id, title, season, episode
                    FROM chapter
                    WHERE
                        hash = ?
                    ORDER BY
                        id
                    "#,
            hash_data,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(if let Some(row) = row_opt {
            Some(ChapterExport {
                id: ChapterId::new(row.id),
                corpus_id: CorpusId::new(row.corpus_id),
                metadata: metadata_from_chapter(row.title, row.season, row.episode),
                hash,
            })
        } else {
            None
        })
    }

    pub async fn get_chapter_by_id(
        &self,
        chapter_id: ChapterId,
    ) -> Result<ChapterExport, DatabaseError> {
        let cid = chapter_id.get();
        let row = sqlx::query!(
            r#"
                    SELECT
                        id, corpus_id, title, season, episode, hash
                    FROM chapter
                    WHERE
                        id = ?
                    "#,
            cid,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(ChapterExport {
            id: ChapterId::new(row.id),
            corpus_id: CorpusId::new(row.corpus_id),
            metadata: metadata_from_chapter(row.title, row.season, row.episode),
            hash: parse_media_hash(&row.hash)?,
        })
    }

    pub async fn get_active_chapters_for_corpus(
        &self,
        corpus_id: CorpusId,
    ) -> Result<Vec<ChapterExport>, DatabaseError> {
        let cid = corpus_id.get();
        let rows = sqlx::query!(
            r#"
                    SELECT
                        id, corpus_id, title, season, episode, hash
                    FROM chapter
                    WHERE
                        corpus_id = ?
                    "#,
            cid,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut chapters = Vec::new();
        for row in rows {
            chapters.push(ChapterExport {
                id: ChapterId::new(row.id),
                corpus_id: CorpusId::new(row.corpus_id),
                metadata: metadata_from_chapter(row.title, row.season, row.episode),
                hash: parse_media_hash(&row.hash)?,
            });
        }
        Ok(chapters)
    }
}

#[cfg(test)]
mod test {

    use lucille_core::{identifiers::ChapterId, metadata::MediaHash};

    use super::*;
    use crate::database_test::assert_err_is_constraint;

    #[tokio::test]
    async fn define_chapter() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        let id = db
            .define_chapter(
                c.id.unwrap(),
                "title",
                Some(1),
                Some(2),
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        assert_eq!(id, ChapterId::new(1))
    }

    #[tokio::test]
    async fn define_chapter_without_episodes() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        let id = db
            .define_chapter(
                c.id.unwrap(),
                "title",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        assert_eq!(id, ChapterId::new(1))
    }

    #[tokio::test]
    async fn define_chapter_without_title() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        assert_err_is_constraint(
            db.define_chapter(
                c.id.unwrap(),
                "",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await,
            "CHECK",
        )
    }

    #[tokio::test]
    async fn define_and_update_chapter() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        let id1 = db
            .define_chapter(
                c.id.unwrap(),
                "title1",
                Some(1),
                Some(1),
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        assert_eq!(id1, ChapterId::new(1));

        let id2 = db
            .define_chapter(
                c.id.unwrap(),
                "title2",
                Some(1),
                Some(2),
                MediaHash::from_bytes(b"data2"),
            )
            .await
            .unwrap();
        assert_eq!(id2, ChapterId::new(2));

        let idx = db
            .define_chapter(
                c.id.unwrap(),
                "title_updated",
                Some(1),
                Some(1),
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        assert_eq!(idx, id1);
    }

    #[tokio::test]
    async fn define_and_redefine_chapter() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        let id1 = db
            .define_chapter(
                c.id.unwrap(),
                "title1",
                Some(1),
                Some(1),
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        assert_eq!(id1, ChapterId::new(1));

        let id2 = db
            .define_chapter(
                c.id.unwrap(),
                "title1",
                Some(1),
                Some(1),
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        assert_eq!(id1, id2);
    }

    #[tokio::test]
    async fn lookup_chapter_by_hash() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        let hash = MediaHash::from_bytes(b"data");
        let id1 = db
            .define_chapter(
                c.id.unwrap(),
                "title1",
                Some(1),
                Some(1),
                MediaHash::from_bytes(b"data1"),
            )
            .await
            .unwrap();
        assert_eq!(id1, ChapterId::new(1));

        let id2 = db
            .define_chapter(c.id.unwrap(), "title2", Some(1), Some(2), hash)
            .await
            .unwrap();
        assert_eq!(id2, ChapterId::new(2));

        let row = db
            .get_chapter_by_hash(hash)
            .await
            .unwrap()
            .expect("obj not in db");
        assert_eq!(row.id, id2);
        assert_eq!(row.corpus_id, c.id.unwrap());
        assert_eq!(row.metadata.title(), "title2");
    }

    #[tokio::test]
    async fn lookup_chapter_by_id() {
        let db = Database::memory().await.unwrap();
        let c = db.add_corpus("media").await.unwrap();
        let hash = MediaHash::from_bytes(b"data");
        let id1 = db
            .define_chapter(
                c.id.unwrap(),
                "title1",
                Some(1),
                Some(1),
                MediaHash::from_bytes(b"data1"),
            )
            .await
            .unwrap();
        assert_eq!(id1, ChapterId::new(1));

        let id2 = db
            .define_chapter(c.id.unwrap(), "title2", Some(1), Some(2), hash)
            .await
            .unwrap();
        assert_eq!(id2, ChapterId::new(2));

        let row = db.get_chapter_by_id(id2).await.unwrap();
        assert_eq!(row.id, id2);
        assert_eq!(row.corpus_id, c.id.unwrap());
        assert_eq!(row.metadata.title(), "title2");
        assert_eq!(row.hash, hash);
    }

    #[tokio::test]
    async fn get_all_active_chapters_for_corpus_only_selects_my_corpus() {
        let db = Database::memory().await.unwrap();
        let c1 = db.add_corpus("media_good").await.unwrap();
        let c2 = db.add_corpus("media_bad").await.unwrap();

        let c1id1 = db
            .define_chapter(
                c1.id.unwrap(),
                "title1",
                Some(1),
                Some(1),
                MediaHash::from_bytes(b"data1"),
            )
            .await
            .unwrap();

        let c1id2 = db
            .define_chapter(
                c1.id.unwrap(),
                "title2",
                Some(1),
                Some(2),
                MediaHash::from_bytes(b"data2"),
            )
            .await
            .unwrap();

        let _ = db
            .define_chapter(
                c2.id.unwrap(),
                "title3",
                Some(2),
                Some(1),
                MediaHash::from_bytes(b"data3"),
            )
            .await
            .unwrap();

        let chapters = db
            .get_active_chapters_for_corpus(c1.id.unwrap())
            .await
            .unwrap();
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].id, c1id1);
        assert_eq!(chapters[1].id, c1id2);
    }
}

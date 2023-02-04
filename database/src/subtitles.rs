use std::collections::{HashMap, HashSet};

use futures::TryStreamExt;
use lucile_core::{
    identifiers::{ChapterId, CorpusId},
    metadata::{EpisodeMetadata, MediaHash, MediaMetadata},
    uuid::Uuid,
    ContentData, Subtitle,
};

use crate::{media_hash, metadata_from_chapter, parse_uuid, Database, DatabaseError};

fn deserialize_subtitle(data: &[u8]) -> Result<Vec<Subtitle>, DatabaseError> {
    serde_json::from_slice(&data)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("deserialize JSON: {}", e)))
}

impl Database {
    pub async fn add_subtitles(
        &self,
        chapter_id: ChapterId,
        subtitles: &[Subtitle],
    ) -> Result<Uuid, DatabaseError> {
        let cid = chapter_id.get();
        let srt_uuid = Uuid::generate();
        let srt_uuid_string = srt_uuid.to_string();
        let data = serde_json::to_vec(subtitles).expect("unable to serialize JSON");
        let id = sqlx::query!(
            r#"
                    INSERT INTO srtfile (chapter_id, uuid, data)
                    VALUES ( ?1, ?2, ?3 )
                    "#,
            cid,
            srt_uuid_string,
            data,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        log::info!("srt file id: {:?} {}", id, srt_uuid);

        Ok(srt_uuid)
    }

    #[deprecated]
    pub async fn get_all_subs_for_corpus(
        &self,
        corpus_id: CorpusId,
    ) -> Result<(HashSet<i64>, Vec<ContentData>), DatabaseError> {
        log::warn!("deprecated function that tries to grab the latest srt for files");

        let cid = corpus_id.get();

        let mut collector = HashMap::new();

        let mut rows = sqlx::query!(
            r#"
            SELECT 
                chapter.id, chapter.title, chapter.season, chapter.episode, chapter.hash
            FROM 
                chapter
            WHERE
                chapter.corpus_id = ?
         "#,
            cid
        )
        .map(|r| {
            (
                r.id,
                r.hash,
                metadata_from_chapter(r.title, r.season, r.episode),
            )
        })
        .fetch(&self.pool);

        while let Some(row) = rows.try_next().await.unwrap() {
            let hash = media_hash(&row.1)?;
            collector.insert(row.0, (hash, row.2));
        }

        let mut results = Vec::with_capacity(collector.len());

        let mut rows = sqlx::query!(
            r#"
                SELECT 
                    srtfile.id,
                    srtfile.uuid,
                    srtfile.chapter_id,
                    srtfile.data
                FROM srtfile
                JOIN chapter
                  ON srtfile.chapter_id = chapter.id
                WHERE 
                  chapter.corpus_id = ? AND
                  srtfile.id in
                    (
                      SELECT 
                        MAX(srtfile.id) 
                      FROM srtfile
                      JOIN chapter
                        ON srtfile.chapter_id = chapter.id
                      GROUP BY chapter.id
                    )
                ORDER BY
                  srtfile.id ASC
         "#,
            cid
        )
        // .map(|r| (r.id, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch(&self.pool);

        let mut collected_srts = HashSet::default();
        while let Some(row) = rows.try_next().await.unwrap() {
            let subtitle = deserialize_subtitle(&row.data)?;
            let uuid = parse_uuid(&row.uuid)?;
            if let Some((hash, metadata)) = collector.remove(&row.chapter_id) {
                results.push(ContentData {
                    metadata,
                    hash,
                    local_id: row.id as u64,
                    global_id: uuid,
                    subtitle,
                });
                collected_srts.insert(row.id);
            } else {
                log::error!(
                    "we have subtitles for an episode `{}`, that we do not have metadata for",
                    row.chapter_id
                )
            }
        }

        for (id, metadata) in collector {
            log::warn!("no subtitles found for chapter_id={}: {:?}", id, metadata);
        }

        Ok((collected_srts, results))
    }

    pub async fn get_all_subs_for_srt(&self, srt_id: i64) -> Result<Vec<Subtitle>, DatabaseError> {
        let record = sqlx::query!(
            r#"
                SELECT 
                    srtfile.data
                FROM srtfile
                WHERE
                  srtfile.id = ?
         "#,
            srt_id,
        )
        // .map(|r| (r.id, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch_one(&self.pool)
        .await?;

        let subs = deserialize_subtitle(&record.data)?;
        Ok(subs)
    }

    // TODO we should not use numeric ids, or this should be better baked into the index schema?
    pub async fn get_episode_by_id(
        &self,
        srt_id: i64,
    ) -> Result<(MediaHash, MediaMetadata), DatabaseError> {
        let ret = sqlx::query!(
            r#"
                SELECT 
                    chapter.id, chapter.title, chapter.season, chapter.episode,
                    chapter.hash
                FROM chapter
                JOIN srtfile
                  ON srtfile.chapter_id = chapter.id
                WHERE 
                  srtfile.id = ?
         "#,
            srt_id
        )
        .map(|r| (r.hash, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch_one(&self.pool)
        .await?;
        // todo custom struct
        Ok((media_hash(&ret.0)?, ret.1))
    }
}

#[cfg(test)]
mod test {
    use lucile_core::{metadata::MediaHash, Subtitle};

    use super::*;

    const SUB1: &str = include_str!("../test_data/srt1.srt");
    const SUB2: &str = include_str!("../test_data/srt2.srt");

    fn parse_subs(srt: &str) -> Vec<Subtitle> {
        subrip::parse(srt).expect("test SRT failed to parse")
    }

    #[tokio::test]
    async fn add_subs() {
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

        let s1 = parse_subs(SUB1);
        let _u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
    }

    #[tokio::test]
    async fn update_subs() {
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

        let s1 = parse_subs(SUB1);
        let s2 = parse_subs(SUB2);
        let u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
        let u2 = db.add_subtitles(ch_id, &s2).await.unwrap();
        assert_ne!(u1, u2)
    }

    #[tokio::test]
    async fn dup_subs_in_different_media() {
        let db = Database::memory().await.unwrap();
        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id1 = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data"),
            )
            .await
            .unwrap();
        let ch_id2 = db
            .define_chapter(
                corpus.id.unwrap(),
                "c1",
                None,
                None,
                MediaHash::from_bytes(b"data2"),
            )
            .await
            .unwrap();

        let s1 = parse_subs(SUB1);
        let u1 = db.add_subtitles(ch_id1, &s1).await.unwrap();
        let u2 = db.add_subtitles(ch_id2, &s1).await.unwrap();
        assert_ne!(u1, u2)
    }

    #[tokio::test]
    async fn dup_subs_in_same_media() {
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

        let s1 = parse_subs(SUB1);
        let _u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
        let _u2 = db.add_subtitles(ch_id, &s1).await.unwrap();
        // TODO this is the desired behavior!
        // assert_eq!(u1, u2)
    }
}

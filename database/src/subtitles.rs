use std::collections::{HashMap, HashSet};

use futures::TryStreamExt;
use lucile_core::{
    identifiers::{ChapterId, CorpusId},
    metadata::{MediaHash, MediaMetadata},
    uuid::Uuid,
    ContentData, LucileSub, Subtitle,
};

use crate::{metadata_from_chapter, parse_media_hash, parse_uuid, Database, DatabaseError};

fn deserialize_subtitle(data: &[u8]) -> Result<Vec<Subtitle>, DatabaseError> {
    serde_json::from_slice(data)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("deserialize JSON: {}", e)))
}

impl Database {
    pub async fn add_subtitles(
        &self,
        chapter_id: ChapterId,
        subtitles: &[Subtitle],
    ) -> Result<Uuid, DatabaseError> {
        if let Some(latest) = self.lookup_latest_sub_for_chapter(chapter_id).await? {
            if latest.subs == subtitles {
                return Ok(latest.uuid);
            }
        }

        let cid = chapter_id.get();
        let srt_uuid = Uuid::generate();
        let srt_uuid_string = srt_uuid.to_string();
        let data = serde_json::to_vec(subtitles).expect("unable to serialize JSON");
        sqlx::query!(
            r#"
                    INSERT INTO srtfile (chapter_id, uuid, data)
                    VALUES ( ?1, ?2, ?3 )
                    "#,
            cid,
            srt_uuid_string,
            data,
        )
        .execute(&self.pool)
        .await?;
        Ok(srt_uuid)
    }

    pub async fn import_subtitles(
        &self,
        chapter_id: ChapterId,
        srt_uuid: Uuid,
        subtitles: &[Subtitle],
    ) -> Result<Uuid, DatabaseError> {
        let cid = chapter_id.get();
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
            let hash = parse_media_hash(&row.1)?;
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
            let lucile_sub = LucileSub {
                id: row.id,
                uuid,
                subs: subtitle,
            };
            if let Some((hash, metadata)) = collector.remove(&row.chapter_id) {
                results.push(ContentData {
                    metadata,
                    hash,
                    subtitle: lucile_sub,
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

    pub async fn get_all_subs_for_srt_by_uuid(
        &self,
        uuid: Uuid,
    ) -> Result<Vec<Subtitle>, DatabaseError> {
        let uuid_str = uuid.to_string();
        let record = sqlx::query!(
            r#"
                SELECT 
                    srtfile.data
                FROM srtfile
                WHERE
                  srtfile.uuid = ?
         "#,
            uuid_str,
        )
        // .map(|r| (r.id, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch_one(&self.pool)
        .await?;

        let subs = deserialize_subtitle(&record.data)?;
        Ok(subs)
    }

    pub async fn lookup_latest_sub_for_chapter(
        &self,
        chapter_id: ChapterId,
    ) -> Result<Option<LucileSub>, DatabaseError> {
        let ch_id = chapter_id.get();
        let opt_row = sqlx::query!(
            r#"
                SELECT
                    srtfile.id, srtfile.uuid, srtfile.data
                FROM srtfile
                WHERE
                  srtfile.chapter_id = ?
                ORDER BY srtfile.id DESC
                LIMIT 1
         "#,
            ch_id,
        )
        // .map(|r| (r.id, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch_optional(&self.pool)
        .await?;

        if let Some(record) = opt_row {
            let subs = deserialize_subtitle(&record.data)?;
            let global_id = parse_uuid(&record.uuid)?;
            Ok(Some(LucileSub {
                id: record.id,
                uuid: global_id,
                subs,
            }))
        } else {
            Ok(None)
        }
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
        Ok((parse_media_hash(&ret.0)?, ret.1))
    }

    /// Translate an srt_id provided by the search index into its Uuid
    pub async fn get_srt_uuid_by_id(&self, srt_id: i64) -> Result<Uuid, DatabaseError> {
        let row = sqlx::query!(
            r#"
                SELECT 
                    uuid
                FROM srtfile
                WHERE 
                  id = ?
         "#,
            srt_id,
        )
        .fetch_one(&self.pool)
        .await?;
        parse_uuid(&row.uuid)
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
    async fn dereference_sub_id_to_uuid() {
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
        let u1 = db.add_subtitles(ch_id, &s1).await.unwrap();

        // There just isn't a better way to get the `srt_id` right now, because its
        // only used while translating for search indexes
        let all_subs = db
            .get_all_subs_for_corpus(corpus.id.unwrap())
            .await
            .unwrap();
        let sub_meta = &all_subs.1[0].subtitle;
        assert_eq!(sub_meta.uuid, u1); // make sure we are setup correctly
                                       //
        let actual_uuid = db.get_srt_uuid_by_id(sub_meta.id).await.unwrap();
        assert_eq!(actual_uuid, u1);
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
        let u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
        let u2 = db.add_subtitles(ch_id, &s1).await.unwrap();
        assert_eq!(u1, u2)
    }

    #[tokio::test]
    async fn lookup_latest_sub() {
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

        let no_subs = db.lookup_latest_sub_for_chapter(ch_id).await.unwrap();
        assert_eq!(no_subs, None);

        let s1 = parse_subs(SUB1);
        let u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
        let u1_subs = db
            .lookup_latest_sub_for_chapter(ch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(u1_subs.uuid, u1);
        assert_eq!(u1_subs.subs, s1);

        let s2 = parse_subs(SUB2);
        let u2 = db.add_subtitles(ch_id, &s2).await.unwrap();
        let u2_subs = db
            .lookup_latest_sub_for_chapter(ch_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(u2_subs.uuid, u2);
        assert_eq!(u2_subs.subs, s2);
    }
}

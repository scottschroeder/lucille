use std::time::Duration;

use lucille_core::{
    encryption_config::KeyData,
    identifiers::{CorpusId, MediaSegmentId, MediaViewId},
    media_segment::MediaSegment,
    metadata::MediaHash,
};

use crate::{parse_media_hash, Database, DatabaseError};

fn parse_duration(text: &str) -> Result<Duration, DatabaseError> {
    let f = text.parse::<f64>().map_err(|_e| {
        DatabaseError::ConvertFromSqlError(format!("unable to parse f64: `{}`", text))
    })?;
    Ok(Duration::from_secs_f64(f))
}

fn parse_encryption_key(text: &str) -> Result<KeyData, DatabaseError> {
    text.parse::<KeyData>().map_err(|e| {
        DatabaseError::ConvertFromSqlError(format!("unable to parse KeyData: `{}`: {}", text, e))
    })
}

struct DBMediaSegment {
    id: i64,
    media_view_id: i64,
    start: String,
    hash: String,
    encryption_key: Option<String>,
    seq_id: i64,
}

impl TryFrom<DBMediaSegment> for MediaSegment {
    type Error = DatabaseError;

    fn try_from(row: DBMediaSegment) -> Result<Self, Self::Error> {
        Ok(MediaSegment {
            id: MediaSegmentId::new(row.id),
            media_view_id: MediaViewId::new(row.media_view_id),
            hash: parse_media_hash(&row.hash)?,
            start: parse_duration(&row.start)?,
            key: row
                .encryption_key
                .map(|s| parse_encryption_key(&s))
                .transpose()?,
        })
    }
}

impl Database {
    pub async fn get_media_segment_by_hash(
        &self,
        hash: MediaHash,
    ) -> Result<Option<MediaSegment>, DatabaseError> {
        let hash_data = hash.to_string();
        sqlx::query_as!(
            DBMediaSegment,
            r#"
                    SELECT
                        id, media_view_id, hash, start, encryption_key, seq_id
                    FROM media_segment
                    WHERE
                        hash = ?
                    "#,
            hash_data,
        )
        .fetch_optional(&self.pool)
        .await?
        .map(MediaSegment::try_from)
        .transpose()
    }

    pub async fn add_media_segment(
        &self,
        media_view_id: MediaViewId,
        sequence_id: u16,
        hash: MediaHash,
        start: Duration,
        key: Option<KeyData>,
    ) -> Result<MediaSegmentId, DatabaseError> {
        let cid = media_view_id.get();
        let hash_data = hash.to_string();
        let tstart = start.as_secs_f64();
        let key_str = key.map(|k| k.to_string());

        let id = sqlx::query!(
            r#"
                    INSERT INTO media_segment (media_view_id, seq_id, hash, start, encryption_key)
                    VALUES ( ?1, ?2, ?3, ?4, ?5)
                    "#,
            cid,
            sequence_id,
            hash_data,
            tstart,
            key_str,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(MediaSegmentId::new(id))
    }

    pub async fn get_media_segment_by_view(
        &self,
        media_view_id: MediaViewId,
    ) -> Result<Vec<MediaSegment>, DatabaseError> {
        let media_id = media_view_id.get();
        sqlx::query_as!(
            DBMediaSegment,
            r#"
                    SELECT
                        id, media_view_id, start, hash, encryption_key, seq_id
                    FROM media_segment
                    WHERE
                        media_view_id = ?
                    ORDER BY
                        seq_id
                    "#,
            media_id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .enumerate()
        .map(|(idx, row)| {
            let seq_id = row.seq_id as usize;
            let segment = row.try_into();
            if seq_id != idx {
                panic!(
                    // log::warn!(
                    "reading media_segment: {:?}: seq_id={} != idx={}",
                    segment, seq_id, idx,
                );
            }
            segment
        })
        .collect()
    }

    /// Get all media segments that match a media_view by name
    /// from within a single corpus
    pub async fn get_media_segments_by_view_name_across_corpus(
        &self,
        corpus_id: CorpusId,
        view_name: &str,
    ) -> Result<Vec<MediaSegment>, DatabaseError> {
        let cid = corpus_id.get();
        sqlx::query_as!(
            DBMediaSegment,
            r#"
                    SELECT
                        ms.id, media_view_id, start, ms.hash, encryption_key, seq_id
                    FROM media_segment as ms
                    JOIN media_view ON ms.media_view_id = media_view.id
                    JOIN chapter ON media_view.chapter_id = chapter.id
                    JOIN corpus ON chapter.corpus_id = corpus.id
                    WHERE
                        media_view.name = ?
                        AND corpus.id = ?
                    ORDER BY
                        ms.id
                    "#,
            view_name,
            cid,
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(MediaSegment::try_from)
        .collect()
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use lucille_core::{encryption_config::SimpleKeyNonce, metadata::MediaHash};

    use super::*;
    use crate::database_test::assert_err_is_constraint;

    fn create_key() -> KeyData {
        KeyData::EasyAesGcmInMemory(SimpleKeyNonce {
            key: "DEADBEEF".as_bytes().to_vec(),
            nonce: "CAFEB00K".as_bytes().to_vec(),
        })
    }

    #[tokio::test]
    async fn add_media_segment() {
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
        let media_view_id = db.add_media_view(ch_id, "test-view").await.unwrap();
        db.add_media_segment(
            media_view_id.id,
            0,
            MediaHash::from_bytes(b"s1data"),
            Duration::from_secs_f64(1.2),
            None,
        )
        .await
        .unwrap();
        db.add_media_segment(
            media_view_id.id,
            1,
            MediaHash::from_bytes(b"s2data"),
            Duration::from_secs_f64(10.3),
            Some(create_key()),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn add_and_fetch_media_segment_by_hash() {
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
        let media_view_id = db.add_media_view(ch_id, "test-view").await.unwrap();
        let s1 = db
            .add_media_segment(
                media_view_id.id,
                0,
                MediaHash::from_bytes(b"s1data"),
                Duration::from_secs_f64(1.2),
                Some(create_key()),
            )
            .await
            .unwrap();
        let _s2 = db
            .add_media_segment(
                media_view_id.id,
                1,
                MediaHash::from_bytes(b"s2data"),
                Duration::from_secs_f64(10.3),
                None,
            )
            .await
            .unwrap();
        let segment = db
            .get_media_segment_by_hash(MediaHash::from_bytes(b"s1data"))
            .await
            .unwrap();
        let segment = segment.expect("expected data to exist in db");
        assert_eq!(segment.id, s1);
        assert_eq!(segment.start, Duration::from_secs_f64(1.2));
        assert_eq!(segment.key, Some(create_key()));
    }

    #[tokio::test]
    async fn get_media_segments_for_view() {
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

        let media_view_id = db.add_media_view(ch_id, "test-view").await.unwrap();
        let s0 = db
            .add_media_segment(
                media_view_id.id,
                0,
                MediaHash::from_bytes(b"s1data0"),
                Duration::from_secs_f64(1.2),
                None,
            )
            .await
            .unwrap();

        let s1 = db
            .add_media_segment(
                media_view_id.id,
                1,
                MediaHash::from_bytes(b"s2data1"),
                Duration::from_secs_f64(10.3),
                Some(create_key()),
            )
            .await
            .unwrap();

        let other_view_id = db.add_media_view(ch_id, "bad-view").await.unwrap();
        db.add_media_segment(
            other_view_id.id,
            0,
            MediaHash::from_bytes(b"s1bad0"),
            Duration::from_secs_f64(0.0),
            None,
        )
        .await
        .unwrap();

        let segments = db
            .get_media_segment_by_view(media_view_id.id)
            .await
            .unwrap();

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].id, s0);
        assert_eq!(segments[1].id, s1);
    }

    #[tokio::test]
    async fn get_media_segments_for_corpus() {
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

        let media_view_id = db.add_media_view(ch_id, "test-view").await.unwrap();
        let s0 = db
            .add_media_segment(
                media_view_id.id,
                0,
                MediaHash::from_bytes(b"s1data0"),
                Duration::from_secs_f64(1.2),
                None,
            )
            .await
            .unwrap();

        let s1 = db
            .add_media_segment(
                media_view_id.id,
                1,
                MediaHash::from_bytes(b"s2data1"),
                Duration::from_secs_f64(10.3),
                Some(create_key()),
            )
            .await
            .unwrap();

        let other_view_id = db.add_media_view(ch_id, "bad-view").await.unwrap();
        db.add_media_segment(
            other_view_id.id,
            0,
            MediaHash::from_bytes(b"s1bad0"),
            Duration::from_secs_f64(0.0),
            None,
        )
        .await
        .unwrap();

        let ch2_id = db
            .define_chapter(
                corpus.id.unwrap(),
                "c2",
                None,
                None,
                MediaHash::from_bytes(b"data2"),
            )
            .await
            .unwrap();

        let media_view_id_ch2 = db.add_media_view(ch2_id, "test-view").await.unwrap();
        let s2 = db
            .add_media_segment(
                media_view_id_ch2.id,
                0,
                MediaHash::from_bytes(b"c2s1data0"),
                Duration::from_secs_f64(1.2),
                None,
            )
            .await
            .unwrap();

        let segments = db
            .get_media_segments_by_view_name_across_corpus(corpus.id.unwrap(), "test-view")
            .await
            .unwrap();

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].id, s0);
        assert_eq!(segments[1].id, s1);
        assert_eq!(segments[2].id, s2);
    }

    #[tokio::test]
    async fn add_media_segments_with_same_sequence_id() {
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
        let media_view_id = db.add_media_view(ch_id, "test-view").await.unwrap();

        db.add_media_segment(
            media_view_id.id,
            0,
            MediaHash::from_bytes(b"s1data"),
            Duration::from_secs_f64(1.2),
            None,
        )
        .await
        .unwrap();
        let result = db
            .add_media_segment(
                media_view_id.id,
                0,
                MediaHash::from_bytes(b"s2data"),
                Duration::from_secs_f64(10.3),
                Some(create_key()),
            )
            .await;

        assert_err_is_constraint(result, "UNIQUE");
    }
}

use std::time::Duration;

use lucile_core::{
    identifiers::{MediaSegmentId, MediaViewId},
    media_segment::{EncryptionKey, MediaSegment},
    metadata::MediaHash,
};

use crate::{Database, DatabaseError};

fn parse_duration(text: &str) -> Result<Duration, DatabaseError> {
    let f = text.parse::<f64>().map_err(|_e| {
        DatabaseError::ConvertFromSqlError(format!("unable to parse f64: `{}`", text))
    })?;
    Ok(Duration::from_secs_f64(f))
}

impl Database {
    pub async fn get_media_segment_by_hash(
        &self,
        hash: MediaHash,
    ) -> Result<Option<MediaSegment>, DatabaseError> {
        let hash_data = hash.to_string();
        let row_opt = sqlx::query!(
            r#"
                    SELECT
                        id, media_view_id, start, encryption_key
                    FROM media_segment
                    WHERE
                        hash = ?
                    "#,
            hash_data,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(if let Some(row) = row_opt {
            Some(MediaSegment {
                id: MediaSegmentId::new(row.id),
                media_view_id: MediaViewId::new(row.media_view_id),
                hash,
                start: parse_duration(&row.start)?,
                key: row.encryption_key.map(EncryptionKey::new),
            })
        } else {
            None
        })
    }

    pub async fn add_media_segment(
        &self,
        media_view_id: MediaViewId,
        sequence_id: u16,
        hash: MediaHash,
        start: Duration,
        key: Option<String>,
    ) -> Result<MediaSegmentId, DatabaseError> {
        let cid = media_view_id.get();
        let hash_data = hash.to_string();
        let tstart = start.as_secs_f64();

        let id = sqlx::query!(
            r#"
                    INSERT INTO media_segment (media_view_id, seq_id, hash, start, encryption_key)
                    VALUES ( ?1, ?2, ?3, ?4, ?5)
                    "#,
            cid,
            sequence_id,
            hash_data,
            tstart,
            key,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(MediaSegmentId::new(id))
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use lucile_core::metadata::MediaHash;

    use super::*;
    use crate::database_test::assert_err_is_constraint;

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
            Some("foo".to_string()),
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
                None,
            )
            .await
            .unwrap();
        let _s2 = db
            .add_media_segment(
                media_view_id.id,
                1,
                MediaHash::from_bytes(b"s2data"),
                Duration::from_secs_f64(10.3),
                Some("foo".to_string()),
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
    }

    #[tokio::test]
    async fn add_media_segment_with_empty_key() {
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
        assert_err_is_constraint(
            db.add_media_segment(
                media_view_id.id,
                0,
                MediaHash::from_bytes(b"s1data"),
                Duration::from_secs_f64(1.2),
                Some(String::new()),
            )
            .await,
            "CHECK",
        )
    }
}

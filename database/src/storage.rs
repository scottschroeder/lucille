use std::path::{Path, PathBuf};

use lucile_core::{export::MediaStorage, identifiers::StorageId, metadata::MediaHash};

use crate::{parse_media_hash, Database, DatabaseError};

struct DBMediaStorage {
    id: i64,
    hash: String,
    path: String,
}

impl TryFrom<DBMediaStorage> for MediaStorage {
    type Error = DatabaseError;

    fn try_from(row: DBMediaStorage) -> Result<Self, Self::Error> {
        Ok(MediaStorage {
            id: StorageId::new(row.id),
            hash: parse_media_hash(&row.hash)?,
            path: PathBuf::from(row.path),
            exists_locally: None,
            verified: false,
        })
    }
}

impl Database {
    pub async fn add_storage(
        &self,
        hash: MediaHash,
        path: &Path,
    ) -> Result<StorageId, DatabaseError> {
        let hash_data = hash.to_string();
        // let path_repr = path.as_os_str().as_bytes();
        let path_repr = path.as_os_str().to_str().expect("path was not valid utf8"); // TODO
        let id = sqlx::query!(
            r#"
                    INSERT INTO storage (hash, path)
                    VALUES ( ?1, ?2)
                    "#,
            hash_data,
            path_repr,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(StorageId::new(id))
    }

    pub async fn get_storage_by_hash(
        &self,
        hash: MediaHash,
    ) -> Result<Option<MediaStorage>, DatabaseError> {
        let hash_data = hash.to_string();
        sqlx::query_as!(
            DBMediaStorage,
            r#"
                    SELECT
                        id, hash, path
                    FROM storage
                    WHERE
                        hash = ?
                    "#,
            hash_data,
        )
        .fetch_optional(&self.pool)
        .await?
        .map(MediaStorage::try_from)
        .transpose()
    }

    pub async fn get_storage_by_path(
        &self,
        path: &std::path::Path,
    ) -> Result<Option<MediaStorage>, DatabaseError> {
        let path_repr = path.as_os_str().to_str().expect("path was not valid utf8"); // TODO
        let row_opt = sqlx::query!(
            r#"
                    SELECT
                        id, hash, path
                    FROM storage
                    WHERE
                        path = ?
                    "#,
            path_repr,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(if let Some(row) = row_opt {
            let hash = parse_media_hash(&row.hash)?;
            Some(MediaStorage {
                id: StorageId::new(row.id),
                hash,
                path: PathBuf::from(row.path),
                exists_locally: None,
                verified: false,
            })
        } else {
            None
        })
    }

    /// Get all elements from storage that have no associated media_segment or chapter
    pub async fn get_storage_orphans(&self) -> Result<Vec<MediaStorage>, DatabaseError> {
        sqlx::query_as!(
            DBMediaStorage,
            r#"
                SELECT 
                    storage.id, storage.hash, storage.path
                FROM storage
                LEFT JOIN media_segment
                    ON storage.hash = media_segment.hash
                LEFT JOIN chapter
                    ON storage.hash = chapter.hash
                WHERE media_segment.id IS NULL
                    AND chapter.id IS NULL
            "#,
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(MediaStorage::try_from)
        .collect()
    }

    /// Delete a storage item by Id
    /// Does not delete any files, this is purely a db operation.
    pub async fn delete_storage(&self, storage_id: StorageId) -> Result<(), DatabaseError> {
        let id = storage_id.get();

        sqlx::query!(
            r#"
            DELETE FROM storage
            WHERE id = ?
            "#,
            id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use lucile_core::metadata::MediaHash;

    use super::*;
    use crate::database_test::assert_err_is_constraint;

    #[tokio::test]
    async fn add_storage() {
        let db = Database::memory().await.unwrap();
        db.add_storage(
            MediaHash::from_bytes(b"s1data"),
            std::path::Path::new("loc/to/path"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn delete_storage() {
        let db = Database::memory().await.unwrap();

        let hash = MediaHash::from_bytes(b"s1data");
        let id = db
            .add_storage(hash, std::path::Path::new("loc/to/path"))
            .await
            .unwrap();

        assert!(db.get_storage_by_hash(hash).await.unwrap().is_some());
        db.delete_storage(id).await.unwrap();
        assert!(db.get_storage_by_hash(hash).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn two_objects_at_the_same_path() {
        let db = Database::memory().await.unwrap();
        db.add_storage(
            MediaHash::from_bytes(b"s1data"),
            std::path::Path::new("loc/to/path"),
        )
        .await
        .unwrap();

        assert_err_is_constraint(
            db.add_storage(
                MediaHash::from_bytes(b"s2data"),
                std::path::Path::new("loc/to/path"),
            )
            .await,
            "UNIQUE",
        )
    }

    #[tokio::test]
    async fn lookup_storage_by_hash() {
        let db = Database::memory().await.unwrap();
        let hash = MediaHash::from_bytes(b"s1data");
        let fpath = std::path::PathBuf::from("loc/to/path");
        db.add_storage(hash, fpath.as_path()).await.unwrap();

        let res_opt = db.get_storage_by_hash(hash).await.unwrap();
        let res = res_opt.expect("expected object not in db");
        assert_eq!(res.path, fpath);
    }
    #[tokio::test]
    async fn lookup_storage_by_path() {
        let db = Database::memory().await.unwrap();
        let hash = MediaHash::from_bytes(b"s1data");
        let fpath = std::path::PathBuf::from("loc/to/path");
        db.add_storage(hash, fpath.as_path()).await.unwrap();

        let res = db
            .get_storage_by_path(fpath.as_path())
            .await
            .unwrap()
            .expect("expected object not in db");
        assert_eq!(res.path, fpath);
    }

    #[tokio::test]
    async fn test_orphans() {
        let db = Database::memory().await.unwrap();

        let chapter_hash = MediaHash::from_bytes(b"chapter");
        let view_hash = MediaHash::from_bytes(b"view");
        let orphan_hash = MediaHash::from_bytes(b"orphan");

        let corpus = db.add_corpus("media").await.unwrap();
        let ch_id = db
            .define_chapter(corpus.id.unwrap(), "c1", None, None, chapter_hash)
            .await
            .unwrap();
        let media_view_id = db.add_media_view(ch_id, "test-view").await.unwrap();

        db.add_media_segment(
            media_view_id.id,
            0,
            view_hash,
            std::time::Duration::from_secs_f64(1.2),
            None,
        )
        .await
        .unwrap();

        db.add_storage(chapter_hash, Path::new("/media/chapter"))
            .await
            .unwrap();
        db.add_storage(view_hash, Path::new("/media/view"))
            .await
            .unwrap();

        let orphan_path = PathBuf::from("/media/orphan");
        let id = db.add_storage(orphan_hash, &orphan_path).await.unwrap();

        let orphans = db.get_storage_orphans().await.unwrap();
        assert_eq!(
            orphans,
            vec![MediaStorage {
                id,
                path: orphan_path,
                hash: orphan_hash,
                exists_locally: None,
                verified: false,
            }]
        );
    }
}

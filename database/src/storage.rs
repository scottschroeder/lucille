use std::path::{Path, PathBuf};

use lucile_core::{export::MediaStorage, identifiers::StorageId, metadata::MediaHash};

use crate::{media_hash, Database, DatabaseError};

impl Database {
    pub async fn add_storage(&self, hash: MediaHash, path: &Path) -> Result<(), DatabaseError> {
        let hash_data = hash.to_string();
        // let path_repr = path.as_os_str().as_bytes();
        let path_repr = path.as_os_str().to_str().expect("path was not valid utf8"); // TODO
        let _id = sqlx::query!(
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

        Ok(())
    }

    pub async fn get_storage_by_hash(
        &self,
        hash: MediaHash,
    ) -> Result<Option<MediaStorage>, DatabaseError> {
        let hash_data = hash.to_string();
        let row_opt = sqlx::query!(
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
        .await?;

        Ok(if let Some(row) = row_opt {
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
            let hash = media_hash(&row.hash)?;
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
}

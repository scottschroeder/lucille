use std::path::Path;

use lucile_core::metadata::MediaHash;

use crate::{Database, DatabaseError};

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
}

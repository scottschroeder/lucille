use std::{path::PathBuf, str::FromStr, time::Duration};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Sqlite,
};

use super::DatabaseSource;
use crate::DatabaseError;

const POOL_TIMEOUT: Duration = Duration::from_secs(30);
const POOL_MAX_CONN: u32 = 2;

#[derive(Debug)]
pub struct LucileDbConnectOptions {
    inner: SqliteConnectOptions,
    source: DatabaseSource,
}

impl LucileDbConnectOptions {
    pub fn memory() -> LucileDbConnectOptions {
        let mut builder = LucileDbConnectOptions {
            inner: SqliteConnectOptions::from_str("sqlite::memory:")
                .expect("failed to create in memory sqlite database"),
            source: DatabaseSource::Memory,
        }
        .apply_common();
        builder.update(|opt| opt.journal_mode(sqlx::sqlite::SqliteJournalMode::Memory));
        builder
    }

    pub fn from_url(url: &str) -> Result<LucileDbConnectOptions, DatabaseError> {
        if url == "sqlite::memory:" || url == "sqlite://:memory:" {
            Ok(LucileDbConnectOptions::memory())
        } else {
            Ok(LucileDbConnectOptions {
                inner: SqliteConnectOptions::from_str(url)?,
                source: DatabaseSource::Url(url.to_owned()),
            }
            .apply_common())
        }
    }

    pub fn from_path(filename: impl Into<PathBuf>) -> LucileDbConnectOptions {
        let p: PathBuf = filename.into();
        LucileDbConnectOptions {
            inner: SqliteConnectOptions::new().filename(p.as_path()),
            source: DatabaseSource::Path(p),
        }
        .apply_common()
    }

    pub async fn create_pool(&self) -> Result<(Pool<Sqlite>, DatabaseSource), DatabaseError> {
        log::debug!("connecting to sqlite database: {:?}", self.source);
        Ok((
            SqlitePoolOptions::new()
                .max_connections(POOL_MAX_CONN)
                .acquire_timeout(POOL_TIMEOUT)
                .connect_with(self.inner.clone())
                .await?,
            self.source.clone(),
        ))
    }
}
impl LucileDbConnectOptions {
    pub fn create_if_missing(mut self, create: bool) -> LucileDbConnectOptions {
        self.update(|opt| opt.create_if_missing(create));
        self
    }
}

impl LucileDbConnectOptions {
    fn update(&mut self, f: impl FnOnce(SqliteConnectOptions) -> SqliteConnectOptions) {
        let mut swp = LucileDbConnectOptions {
            inner: SqliteConnectOptions::new(),
            source: DatabaseSource::Memory,
        };
        std::mem::swap(&mut swp, self);
        let LucileDbConnectOptions { inner, source } = swp;
        let mut ret = LucileDbConnectOptions {
            inner: f(inner),
            source,
        };
        std::mem::swap(&mut ret, self);
    }

    fn apply_common(mut self) -> LucileDbConnectOptions {
        self.update(|opt| {
            opt.synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                .create_if_missing(true)
                .busy_timeout(POOL_TIMEOUT)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        });
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn create_in_memory_db() {
        let (_pool, src) = LucileDbConnectOptions::memory()
            .create_pool()
            .await
            .unwrap();
        assert_eq!(src, DatabaseSource::Memory);
    }

    #[tokio::test]
    async fn create_in_memory_db_from_url() {
        let (_pool, src) = LucileDbConnectOptions::from_url("sqlite::memory:")
            .unwrap()
            .create_pool()
            .await
            .unwrap();
        assert_eq!(src, DatabaseSource::Memory);
    }

    #[tokio::test]
    async fn create_file_db() {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("test.db");
        let (_pool, src) = LucileDbConnectOptions::from_path(&db_path)
            .create_pool()
            .await
            .unwrap();
        assert_eq!(src, DatabaseSource::Path(db_path));
    }

    #[tokio::test]
    async fn fail_to_create_db_with_create_false() {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("test.db");
        let res = LucileDbConnectOptions::from_path(&db_path)
            .create_if_missing(false)
            .create_pool()
            .await;
        assert!(res.is_err())
    }

    #[tokio::test]
    async fn create_previously_created_db_with_create_false() {
        let root = tempfile::tempdir().unwrap();
        let db_path = root.path().join("test.db");
        {
            LucileDbConnectOptions::from_path(&db_path)
                .create_pool()
                .await
                .unwrap();
        }
        LucileDbConnectOptions::from_path(&db_path)
            .create_if_missing(false)
            .create_pool()
            .await
            .unwrap();
    }
}

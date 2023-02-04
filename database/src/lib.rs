use std::{
    path::{self, PathBuf},
    str::FromStr,
    time::Duration,
};

use futures::TryStreamExt;
use lucile_core::{metadata::MediaHash, uuid::Uuid};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous},
    Pool, Sqlite,
};

const POOL_TIMEOUT: Duration = Duration::from_secs(30);
const POOL_MAX_CONN: u32 = 2;
const DATABASE_ENV_VAR: &str = "DATABASE_URL";

mod chapter;
mod corpus;
mod index;
mod media_segment;
mod media_view;
mod storage;
mod subtitles;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    VarError(#[from] std::env::VarError),
    #[error("must specify a database")]
    NoDatabaseSpecified,
    #[error("unable to convert datatype from sql: {}", _0)]
    ConvertFromSqlError(String),
}

#[derive(Debug)]
pub enum DatabaseSource {
    Memory,
    Env(String),
    Path(PathBuf),
}

#[derive(Debug)]
pub struct Database {
    pool: Pool<Sqlite>,
    source: DatabaseSource,
}

pub fn db_env() -> Result<Option<String>, DatabaseError> {
    match std::env::var(DATABASE_ENV_VAR) {
        Ok(db) => Ok(Some(db)),
        Err(e) => match e {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(_) => Err(DatabaseError::from(e)),
        },
    }
}

impl Database {
    pub async fn memory() -> Result<Database, DatabaseError> {
        let pool = memory_db().await?;
        migrations(&pool).await?;
        Ok(Database {
            pool,
            source: DatabaseSource::Memory,
        })
    }
    pub async fn from_path<P: AsRef<path::Path>>(filename: P) -> Result<Database, DatabaseError> {
        let filename = filename.as_ref();
        let pool = connect_db(filename).await?;
        migrations(&pool).await?;
        Ok(Database {
            pool,
            source: DatabaseSource::Path(filename.to_path_buf()),
        })
    }
    pub async fn from_env() -> Result<Database, DatabaseError> {
        let url = db_env()?.ok_or(DatabaseError::NoDatabaseSpecified)?;
        let pool = from_env_db(&url).await?;
        migrations(&pool).await?;
        Ok(Database {
            pool,
            source: DatabaseSource::Env(url),
        })
    }
}

/*
 *
 * GET ALL SHOWS ASSOCIATED WITH A SEARCH INDEX
 *
SELECT DISTINCT
  corpus.id, corpus.title
FROM corpus
JOIN chapter
  ON chapter.corpus_id = corpus.id
JOIN srtfile
  ON srtfile.chapter_id = chapter.id
JOIN search_assoc
  ON search_assoc.srt_id = srtfile.id
JOIN search_index
  ON search_index.id = search_assoc.search_index_id
WHERE search_index.uuid = "5d0b7314-4136-476a-b91a-4cf0b80bd985"
GROUP BY corpus.title
;
*/

fn media_hash(text: &str) -> Result<MediaHash, DatabaseError> {
    MediaHash::from_str(text)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("invalid hex: {:?}", e)))
}

fn parse_uuid(text: &str) -> Result<Uuid, DatabaseError> {
    Uuid::from_str(text)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("invalid uuid: {:?}", e)))
}

async fn migrations(pool: &Pool<Sqlite>) -> Result<(), DatabaseError> {
    sqlx::migrate!().run(pool).await?;
    Ok(())
}

async fn create_pool(opts: SqliteConnectOptions) -> Result<Pool<Sqlite>, DatabaseError> {
    Ok(SqlitePoolOptions::new()
        .max_connections(POOL_MAX_CONN)
        .acquire_timeout(POOL_TIMEOUT)
        .connect_with(opts)
        .await?)
}

async fn from_env_db(url: &str) -> Result<Pool<Sqlite>, DatabaseError> {
    log::info!("connecting to sqlite db at `{}`", url);
    let opts = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(POOL_TIMEOUT)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    create_pool(opts).await
}
async fn memory_db() -> Result<Pool<Sqlite>, DatabaseError> {
    log::info!("connecting to sqlite db in-memory");
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?
        .create_if_missing(true)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(POOL_TIMEOUT)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Memory);
    create_pool(opts).await
}

async fn connect_db<P: AsRef<path::Path>>(filename: P) -> Result<Pool<Sqlite>, DatabaseError> {
    log::info!("connecting to sqlite db at {:?}", filename.as_ref());
    if let Some(dir) = filename.as_ref().parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            log::error!(
                "unable to create directory {:?} for the database: {}",
                dir,
                e
            );
        }
    }
    let opts = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(POOL_TIMEOUT)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    create_pool(opts).await
}

#[cfg(test)]
pub(crate) mod database_test {
    use futures::TryStreamExt;

    use crate::Database;

    const TABLES: &[&str] = &["_sqlx_migrations", "corpus", "chapter", "srtfile"];

    pub(crate) fn assert_err_is_constraint<T: std::fmt::Debug>(
        result: Result<T, super::DatabaseError>,
        text: &str,
    ) {
        let phrase = format!("{} constraint", text);
        if let Err(crate::DatabaseError::Sqlx(ref e)) = result {
            if e.to_string().contains(&phrase) {
                return;
            }
        }
        panic!("expected error against {}, found: {:?}", phrase, result,)
    }

    #[tokio::test]
    async fn all_tables_exist_in_new_database() {
        let db = Database::memory().await.unwrap();
        let mut rows = sqlx::query!(
            r#"
            SELECT 
                name
            FROM 
                sqlite_schema
            WHERE 
                type ='table' AND 
                name NOT LIKE 'sqlite_%';
         "#
        )
        .fetch(&db.pool);

        let mut seen = Vec::new();
        while let Some(row) = rows.try_next().await.unwrap() {
            if let Some(name) = row.name {
                seen.push(name);
            }
        }
        for expected in TABLES {
            assert!(seen.contains(&expected.to_string()))
        }
    }
}

#![allow(clippy::uninlined_format_args)]
use std::{
    path::{self, PathBuf},
    str::FromStr,
    time::Duration,
};

use lucile_core::{
    metadata::{EpisodeMetadata, MediaHash, MediaMetadata},
    uuid::Uuid,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous},
    Pool, Sqlite,
};

const POOL_TIMEOUT: Duration = Duration::from_secs(30);
const POOL_MAX_CONN: u32 = 2;
pub const DATABASE_ENV_VAR: &str = "DATABASE_URL";

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
    #[error("unable to migrate database {:?}", _0)]
    Migrate(DatabaseSource, #[source] sqlx::migrate::MigrateError),
    #[error(transparent)]
    VarError(#[from] std::env::VarError),
    #[error("must specify a database")]
    NoDatabaseSpecified,
    #[error("unable to convert datatype from sql: {}", _0)]
    ConvertFromSqlError(String),
}

#[derive(Debug, Clone)]
pub enum DatabaseSource {
    Memory,
    Env(String),
    Path(PathBuf),
}

#[derive(Debug)]
pub struct DatabaseFetcher {
    pub db: Database,
    pub source: DatabaseSource,
}

#[derive(Debug, Clone)]
pub struct Database {
    pool: Pool<Sqlite>,
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
        migrations(&DatabaseSource::Memory, &pool).await?;
        Ok(Database { pool })
    }
}

impl DatabaseFetcher {
    pub async fn memory() -> Result<DatabaseFetcher, DatabaseError> {
        Ok(DatabaseFetcher {
            db: Database::memory().await?,
            source: DatabaseSource::Memory,
        })
    }
    pub async fn from_url_or_file(url: String) -> Result<DatabaseFetcher, DatabaseError> {
        if url.starts_with("sqlite:") {
            let pool = from_env_db(&url).await?;
            let source = DatabaseSource::Env(url);
            migrations(&source, &pool).await?;
            Ok(DatabaseFetcher {
                db: Database { pool },
                source,
            })
        } else {
            DatabaseFetcher::from_path(url).await
        }
    }
    pub async fn from_path<P: AsRef<path::Path>>(
        filename: P,
    ) -> Result<DatabaseFetcher, DatabaseError> {
        let filename = filename.as_ref();
        let pool = connect_db(filename).await?;
        let source = DatabaseSource::Path(filename.to_path_buf());
        migrations(&source, &pool).await?;
        Ok(DatabaseFetcher {
            db: Database { pool },
            source,
        })
    }
    pub async fn from_env() -> Result<DatabaseFetcher, DatabaseError> {
        let url = db_env()?.ok_or(DatabaseError::NoDatabaseSpecified)?;
        let pool = from_env_db(&url).await?;
        let source = DatabaseSource::Env(url);
        migrations(&source, &pool).await?;
        Ok(DatabaseFetcher {
            db: Database { pool },
            source,
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

fn metadata_from_chapter(
    title: String,
    season: Option<i64>,
    episode: Option<i64>,
) -> MediaMetadata {
    if let Some((s, e)) = season.zip(episode) {
        MediaMetadata::Episode(EpisodeMetadata {
            title,
            season: s as u32,
            episode: e as u32,
        })
    } else {
        MediaMetadata::Unknown(title)
    }
}

fn parse_media_hash(text: &str) -> Result<MediaHash, DatabaseError> {
    MediaHash::from_str(text)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("invalid hex: {:?}", e)))
}

fn parse_uuid(text: &str) -> Result<Uuid, DatabaseError> {
    Uuid::from_str(text)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("invalid uuid: {:?}", e)))
}

async fn migrations(src: &DatabaseSource, pool: &Pool<Sqlite>) -> Result<(), DatabaseError> {
    sqlx::migrate!()
        .run(pool)
        .await
        .map_err(|e| DatabaseError::Migrate(src.clone(), e))?;
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

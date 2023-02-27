#![allow(clippy::uninlined_format_args)]
use std::{
    path::{self},
    str::FromStr,
    time::Duration,
};

use lucile_core::{
    metadata::{EpisodeMetadata, MediaHash, MediaMetadata},
    uuid::Uuid,
};
use sqlx::{Pool, Sqlite};

pub use self::build::{DatabaseBuider, DatabaseSource, LucileDbConnectOptions};

const POOL_TIMEOUT: Duration = Duration::from_secs(30);
const POOL_MAX_CONN: u32 = 2;
pub const DATABASE_ENV_VAR: &str = "DATABASE_URL";

mod build;
mod chapter;
mod corpus;
mod index;
mod media_segment;
mod media_view;
mod storage;
mod subtitles;

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("state error: {}", _0)]
    ConnectStateError(&'static str),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error(transparent)]
    VarError(#[from] std::env::VarError),
    #[error("must specify a database")]
    NoDatabaseSpecified,
    #[error("unable to convert datatype from sql: {}", _0)]
    ConvertFromSqlError(String),
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
        let mut builder = DatabaseBuider::default();
        builder.add_opts(LucileDbConnectOptions::memory())?;
        builder.connect().await?;
        builder.migrate().await?;
        let (db, _) = builder.into_parts()?;
        Ok(db)
    }

    pub async fn get_db_migration_status(
        &self,
    ) -> Result<Vec<build::MigrationRecord>, DatabaseError> {
        build::get_db_migration_status(&self.pool).await
    }
}

impl DatabaseFetcher {
    #[deprecated(note = "use stateful `DatabaseBuilder` api")]
    pub async fn memory() -> Result<DatabaseFetcher, DatabaseError> {
        let mut builder = DatabaseBuider::default();
        builder.add_opts(LucileDbConnectOptions::memory())?;
        builder.connect().await?;
        builder.migrate().await?;
        let (db, source) = builder.into_parts()?;
        Ok(DatabaseFetcher { db, source })
    }
    #[deprecated(note = "use stateful `DatabaseBuilder` api")]
    pub async fn from_url_or_file(url: String) -> Result<DatabaseFetcher, DatabaseError> {
        let mut builder = DatabaseBuider::default();
        let opts = if url.starts_with("sqlite:") {
            LucileDbConnectOptions::from_url(&url)?
        } else {
            LucileDbConnectOptions::from_path(&url)
        };
        builder.add_opts(opts)?;
        builder.connect().await?;
        builder.migrate().await?;
        let (db, source) = builder.into_parts()?;
        Ok(DatabaseFetcher { db, source })
    }

    #[deprecated(note = "use stateful `DatabaseBuilder` api")]
    pub async fn from_path<P: AsRef<path::Path>>(
        filename: P,
    ) -> Result<DatabaseFetcher, DatabaseError> {
        let mut builder = DatabaseBuider::default();
        builder.add_opts(LucileDbConnectOptions::from_path(filename.as_ref()))?;
        builder.connect().await?;
        builder.migrate().await?;
        let (db, source) = builder.into_parts()?;
        Ok(DatabaseFetcher { db, source })
    }
    #[deprecated(note = "use stateful `DatabaseBuilder` api")]
    pub async fn from_env() -> Result<DatabaseFetcher, DatabaseError> {
        let url = db_env()?.ok_or(DatabaseError::NoDatabaseSpecified)?;
        let mut builder = DatabaseBuider::default();
        builder.add_opts(LucileDbConnectOptions::from_url(&url)?)?;
        builder.connect().await?;
        builder.migrate().await?;
        let (db, source) = builder.into_parts()?;
        Ok(DatabaseFetcher { db, source })
    }
}

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

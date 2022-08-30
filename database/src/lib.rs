use futures::TryStreamExt;
use lucile_core::metadata::MediaHash;
use lucile_core::{ChapterId, Corpus, CorpusId, Subtitle};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Pool, QueryBuilder, Sqlite};
use std::path;
use std::str::FromStr;
use std::time::Duration;

const POOL_TIMEOUT: Duration = Duration::from_secs(30);
const POOL_MAX_CONN: u32 = 2;
const DATABASE_ENV_VAR: &str = "DATABASE_URL";

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("must specify a database")]
    NoDatabaseSpecified,
}

pub struct Database {
    pool: Pool<Sqlite>,
}

impl Database {
    pub async fn memory() -> Result<Database, DatabaseError> {
        let pool = memory_db().await?;
        migrations(&pool).await?;
        Ok(Database { pool })
    }
    pub async fn from_path<P: AsRef<path::Path>>(filename: P) -> Result<Database, DatabaseError> {
        let pool = connect_db(filename).await?;
        migrations(&pool).await?;
        Ok(Database { pool })
    }
    pub async fn from_env() -> Result<Database, DatabaseError> {
        let url = std::env::var(DATABASE_ENV_VAR).map_err(|e| {
            log::error!(
                "could not load database from ENV var `{}`: {}",
                DATABASE_ENV_VAR,
                e
            );
            DatabaseError::NoDatabaseSpecified
        })?;
        let pool = from_env_db(&url).await?;
        migrations(&pool).await?;
        Ok(Database { pool })
    }

    pub async fn add_corpus<S: Into<String>>(&self, name: S) -> Result<Corpus, DatabaseError> {
        let name = name.into();
        let id = sqlx::query!(
            r#"
                    INSERT INTO corpus (title)
                    VALUES ( ?1 )
                    "#,
            name,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        let cid = CorpusId::new(id);

        Ok(Corpus {
            id: Some(cid),
            title: name,
        })
    }

    pub async fn get_corpus_id(&self, title: &str) -> Result<Option<CorpusId>, DatabaseError> {
        let id = sqlx::query!(
            r#"
            SELECT 
                id
            FROM 
                corpus
            WHERE
                title = ?
         "#,
            title,
        )
        .map(|r| CorpusId::new(r.id))
        .fetch_optional(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn get_or_add_corpus<S: Into<String>>(
        &self,
        name: S,
    ) -> Result<Corpus, DatabaseError> {
        let name = name.into();
        Ok(match self.get_corpus_id(&name).await? {
            Some(id) => Corpus {
                id: Some(id),
                title: name,
            },
            None => self.add_corpus(name).await?,
        })
    }

    pub async fn list_corpus(&self) -> Result<Vec<Corpus>, DatabaseError> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                id, title
            FROM 
                corpus
         "#
        )
        .map(|r| Corpus {
            id: Some(CorpusId::new(r.id)),
            title: r.title,
        })
        .fetch(&self.pool);

        Ok(rows.try_collect().await?)
    }

    pub async fn define_chapter<S: Into<String>>(
        &self,
        corpus_id: CorpusId,
        title: S,
        season: Option<i64>,
        episode: Option<i64>,
        hash: MediaHash,
    ) -> Result<ChapterId, DatabaseError> {
        let title = title.into();
        log::trace!(
            "define chapter: C={}, title={:?}, S[{:?}] E[{:?}] {:?}",
            corpus_id,
            title,
            season,
            episode,
            hash
        );

        let cid = corpus_id.get();
        let hash_data = hash.as_slice();
        let id = sqlx::query!(
            r#"
                    INSERT INTO chapter (corpus_id, title, season, episode, hash)
                    VALUES ( ?1, ?2, ?3, ?4, ?5 )
                    "#,
            cid,
            title,
            season,
            episode,
            hash_data
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        log::info!("chapter_id: {:?}", id);

        Ok(ChapterId::new(id))
    }

    pub async fn add_subtitles(
        &self,
        chapter_id: ChapterId,
        subtitles: &[Subtitle],
    ) -> Result<(), DatabaseError> {
        let cid = chapter_id.get();
        let id = sqlx::query!(
            r#"
                    INSERT INTO srtfile (chapter_id)
                    VALUES ( ?1 )
                    "#,
            cid,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        log::info!("srt file id: {:?}", id);

        let mut insert_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            r#"INSERT INTO subtitle (srt_id, idx, content, time_start, time_end)"#,
        );

        insert_builder.push_values(subtitles.iter().enumerate(), |mut b, (idx, sub)| {
            b.push_bind(id)
                .push_bind(idx as u32)
                .push_bind(sub.text.as_str())
                .push_bind(sub.start.as_secs_f64())
                .push_bind(sub.end.as_secs_f64());
        });
        let query = insert_builder.build();

        query.execute(&self.pool).await?;

        Ok(())
    }
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
    let opts = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(POOL_TIMEOUT)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
    create_pool(opts).await
}

#[cfg(test)]
mod tests {
    use crate::Database;
    use futures::TryStreamExt;

    const TABLES: &[&str] = &["_sqlx_migrations", "corpus", "chapter", "subtitle"];

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
        assert_eq!(seen.as_slice(), TABLES);
    }
}

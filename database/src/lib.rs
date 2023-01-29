use std::{
    collections::{HashMap, HashSet},
    path::{self, Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use futures::TryStreamExt;
use lucile_core::{
    identifiers::{ChapterId, CorpusId, MediaViewId},
    metadata::{EpisodeMetadata, MediaHash, MediaMetadata},
    uuid::Uuid,
    ContentData, Corpus, Subtitle,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteSynchronous},
    Pool, QueryBuilder, Sqlite,
};

const POOL_TIMEOUT: Duration = Duration::from_secs(30);
const POOL_MAX_CONN: u32 = 2;
const DATABASE_ENV_VAR: &str = "DATABASE_URL";

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

    pub async fn get_corpus(&self, id: CorpusId) -> Result<Corpus, DatabaseError> {
        let cid = id.get();
        let corpus = sqlx::query!(
            r#"
            SELECT 
                id, title
            FROM 
                corpus
            WHERE
                id = ?
         "#,
            cid
        )
        .map(|r| Corpus {
            id: Some(CorpusId::new(r.id)),
            title: r.title,
        })
        .fetch_one(&self.pool)
        .await?;
        Ok(corpus)
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
        let hash_data = hash.to_string();
        let c = sqlx::query!(
            r#"
                UPDATE chapter
                SET
                    corpus_id = ?1,
                    title = ?2,
                    season = ?3,
                    episode = ?4
                WHERE
                    hash = ?5
            "#,
            cid,
            title,
            season,
            episode,
            hash_data
        )
        .execute(&self.pool)
        .await?;
        c.rows_affected();

        log::trace!("UPDATE RESULT: {:?}", c);

        let id = if c.rows_affected() > 0 {
            sqlx::query!(
                r#"
                    SELECT
                        id
                    FROM chapter
                    WHERE
                        hash = ?
                "#,
                hash_data
            )
            .fetch_one(&self.pool)
            .await?
            .id
        } else {
            sqlx::query!(
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
            .last_insert_rowid()
        };
        log::trace!("UPDATE ID: {:?}", id);
        Ok(ChapterId::new(id))
    }

    pub async fn add_media_view<S: Into<String>>(
        &self,
        chapter_id: ChapterId,
        name: S,
    ) -> Result<MediaViewId, DatabaseError> {
        let name = name.into();

        let cid = chapter_id.get();
        let id = sqlx::query!(
            r#"
                    INSERT INTO media_view (chapter_id, name)
                    VALUES ( ?1, ?2 )
                    "#,
            cid,
            name,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(MediaViewId::new(id))
    }

    pub async fn add_media_segment(
        &self,
        media_view_id: MediaViewId,
        hash: MediaHash,
        start: Duration,
        end: Duration,
        key: Option<String>,
    ) -> Result<(), DatabaseError> {
        let cid = media_view_id.get();
        let hash_data = hash.to_string();
        let tstart = start.as_secs_f64();
        let tend = end.as_secs_f64();

        let _id = sqlx::query!(
            r#"
                    INSERT INTO media_segment (media_view_id, hash, start, end, encryption_key)
                    VALUES ( ?1, ?2, ?3, ?4, ?5)
                    "#,
            cid,
            hash_data,
            tstart,
            tend,
            key,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        Ok(())
    }

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

    pub async fn add_subtitles(
        &self,
        chapter_id: ChapterId,
        subtitles: &[Subtitle],
    ) -> Result<Uuid, DatabaseError> {
        let cid = chapter_id.get();
        let srt_uuid = Uuid::generate();
        let srt_uuid_string = srt_uuid.to_string();
        let data = serde_json::to_vec(subtitles).expect("unable to serialize JSON");
        let id = sqlx::query!(
            r#"
                    INSERT INTO srtfile (chapter_id, uuid, data)
                    VALUES ( ?1, ?2, ?3 )
                    "#,
            cid,
            srt_uuid_string,
            data,
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        log::info!("srt file id: {:?} {}", id, srt_uuid);

        Ok(srt_uuid)
    }

    #[deprecated]
    pub async fn get_all_subs_for_corpus(
        &self,
        corpus_id: CorpusId,
    ) -> Result<(HashSet<i64>, Vec<ContentData>), DatabaseError> {
        log::warn!("deprecated function that tries to grab the latest srt for files");

        let cid = corpus_id.get();

        let mut collector = HashMap::new();

        let mut rows = sqlx::query!(
            r#"
            SELECT 
                chapter.id, chapter.title, chapter.season, chapter.episode, chapter.hash
            FROM 
                chapter
            WHERE
                chapter.corpus_id = ?
         "#,
            cid
        )
        .map(|r| {
            (
                r.id,
                r.hash,
                metadata_from_chapter(r.title, r.season, r.episode),
            )
        })
        .fetch(&self.pool);

        while let Some(row) = rows.try_next().await.unwrap() {
            let hash = media_hash(&row.1)?;
            collector.insert(row.0, (hash, row.2));
        }

        let mut results = Vec::with_capacity(collector.len());

        let mut rows = sqlx::query!(
            r#"
                SELECT 
                    srtfile.id,
                    srtfile.uuid,
                    srtfile.chapter_id,
                    srtfile.data
                FROM srtfile
                JOIN chapter
                  ON srtfile.chapter_id = chapter.id
                WHERE 
                  chapter.corpus_id = ? AND
                  srtfile.id in
                    (
                      SELECT 
                        MAX(srtfile.id) 
                      FROM srtfile
                      JOIN chapter
                        ON srtfile.chapter_id = chapter.id
                      GROUP BY chapter.id
                    )
                ORDER BY
                  srtfile.id ASC
         "#,
            cid
        )
        // .map(|r| (r.id, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch(&self.pool);

        let mut collected_srts = HashSet::default();
        while let Some(row) = rows.try_next().await.unwrap() {
            let subtitle = deserialize_subtitle(&row.data)?;
            let uuid = parse_uuid(&row.uuid)?;
            if let Some((hash, metadata)) = collector.remove(&row.chapter_id) {
                results.push(ContentData {
                    metadata,
                    hash,
                    local_id: row.id as u64,
                    global_id: uuid,
                    subtitle,
                });
                collected_srts.insert(row.id);
            } else {
                log::error!(
                    "we have subtitles for an episode `{}`, that we do not have metadata for",
                    row.chapter_id
                )
            }
        }

        for (id, metadata) in collector {
            log::warn!("no subtitles found for chapter_id={}: {:?}", id, metadata);
        }

        Ok((collected_srts, results))
    }

    pub async fn assoc_index_with_srts(
        &self,
        index_uuid: Uuid,
        srts: HashSet<i64>,
    ) -> Result<(), DatabaseError> {
        log::debug!(
            "associating {} srt files with search index {}",
            srts.len(),
            index_uuid
        );
        let uuid = index_uuid.to_string();
        let id = sqlx::query!(
            r#"
                    INSERT INTO search_index (uuid)
                    VALUES ( ?1 )
                    "#,
            uuid
        )
        .execute(&self.pool)
        .await?
        .last_insert_rowid();

        let mut insert_builder: QueryBuilder<Sqlite> =
            QueryBuilder::new(r#"INSERT INTO search_assoc (search_index_id, srt_id)"#);

        insert_builder.push_values(srts.iter(), |mut b, srt| {
            b.push_bind(id).push_bind(srt);
        });
        let query = insert_builder.build();

        query.execute(&self.pool).await?;

        Ok(())
    }

    // TODO we should not use numeric ids, or this should be better baked into the index schema?
    pub async fn get_episode_by_id(
        &self,
        srt_id: i64,
    ) -> Result<(MediaHash, MediaMetadata), DatabaseError> {
        let ret = sqlx::query!(
            r#"
                SELECT 
                    chapter.id, chapter.title, chapter.season, chapter.episode,
                    chapter.hash
                FROM chapter
                JOIN srtfile
                  ON srtfile.chapter_id = chapter.id
                WHERE 
                  srtfile.id = ?
         "#,
            srt_id
        )
        .map(|r| (r.hash, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch_one(&self.pool)
        .await?;
        // todo custom struct
        Ok((media_hash(&ret.0)?, ret.1))
    }
    pub async fn get_all_subs_for_srt(&self, srt_id: i64) -> Result<Vec<Subtitle>, DatabaseError> {
        let record = sqlx::query!(
            r#"
                SELECT 
                    srtfile.data
                FROM srtfile
                WHERE
                  srtfile.id = ?
         "#,
            srt_id,
        )
        // .map(|r| (r.id, metadata_from_chapter(r.title, r.season, r.episode)))
        .fetch_one(&self.pool)
        .await?;

        let subs = deserialize_subtitle(&record.data)?;
        Ok(subs)
    }

    pub async fn get_media_view_options(
        &self,
        chapter_id: ChapterId,
    ) -> Result<Vec<(MediaViewId, String)>, DatabaseError> {
        let cid = chapter_id.get();
        let rows = sqlx::query!(
            r#"
                SELECT 
                    id, name
                FROM media_view
                WHERE
                    chapter_id = ?
                ORDER BY
                    id DESC
         "#,
            cid
        )
        .map(|r| (MediaViewId::new(r.id), r.name))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
    pub async fn get_srt_view_options(
        &self,
        srt_uuid: Uuid,
    ) -> Result<Vec<(MediaViewId, String)>, DatabaseError> {
        let uuid = srt_uuid.to_string();
        let rows = sqlx::query!(
            r#"
                SELECT 
                    media_view.id, name
                FROM media_view
                JOIN srtfile
                  ON srtfile.chapter_id = media_view.chapter_id
                WHERE
                    srtfile.uuid = ?
                ORDER BY
                    media_view.id DESC
         "#,
            uuid
        )
        .map(|r| (MediaViewId::new(r.id), r.name))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
    pub async fn get_search_indexes(&self) -> Result<Vec<Uuid>, DatabaseError> {
        let mut rows = sqlx::query!(
            r#"
                SELECT 
                    uuid
                FROM search_index
                ORDER BY
                    id
         "#,
        )
        .map(|r| r.uuid)
        .fetch(&self.pool);

        let mut results = Vec::new();
        while let Some(row) = rows.try_next().await.unwrap() {
            let uuid = parse_uuid(&row)?;
            results.push(uuid);
        }
        Ok(results)
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

fn parse_uuid(text: &str) -> Result<Uuid, DatabaseError> {
    Uuid::from_str(text)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("invalid uuid: {:?}", e)))
}
fn media_hash(text: &str) -> Result<MediaHash, DatabaseError> {
    MediaHash::from_str(text)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("invalid hex: {:?}", e)))
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

fn deserialize_subtitle(data: &[u8]) -> Result<Vec<Subtitle>, DatabaseError> {
    serde_json::from_slice(&data)
        .map_err(|e| DatabaseError::ConvertFromSqlError(format!("deserialize JSON: {}", e)))
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
mod database_test {
    use futures::TryStreamExt;
    use lucile_core::identifiers::CorpusId;

    use crate::Database;

    const TABLES: &[&str] = &["_sqlx_migrations", "corpus", "chapter", "srtfile"];

    fn assert_err_is_constraint<T: std::fmt::Debug>(
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

    mod corpus {
        use super::*;

        #[tokio::test]
        async fn create_new_corpus() {
            let db = Database::memory().await.unwrap();
            let c = db.add_corpus("media").await.unwrap();
            assert_eq!(c.id, Some(CorpusId::new(1)));
            assert_eq!(c.title, "media");
        }

        #[tokio::test]
        async fn create_new_empty_corpus() {
            let db = Database::memory().await.unwrap();
            assert_err_is_constraint(db.add_corpus("").await, "CHECK")
        }

        #[tokio::test]
        async fn fail_to_create_two_identical_corpuses() {
            let db = Database::memory().await.unwrap();
            let c = db.add_corpus("media").await.unwrap();
            assert_eq!(c.id, Some(CorpusId::new(1)));
            assert_eq!(c.title, "media");
            assert_err_is_constraint(db.add_corpus("media").await, "UNIQUE")
        }

        #[tokio::test]
        async fn lookup_corpus_id_from_title() {
            let db = Database::memory().await.unwrap();
            let c1 = db.add_corpus("media").await.unwrap();
            let c2 = db.add_corpus("media2").await.unwrap();
            let cid1 = db.get_corpus_id("media").await.unwrap();
            let cid2 = db.get_corpus_id("media2").await.unwrap();
            assert_eq!(c1.id, cid1);
            assert_eq!(c2.id, cid2);
        }

        #[tokio::test]
        async fn lookup_corpus_from_id() {
            let db = Database::memory().await.unwrap();
            let c1 = db.add_corpus("media").await.unwrap();
            let c2 = db.add_corpus("media2").await.unwrap();
            let c_lookup1 = db.get_corpus(c1.id.unwrap()).await.unwrap();
            let c_lookup2 = db.get_corpus(c2.id.unwrap()).await.unwrap();
            assert_eq!(c1, c_lookup1);
            assert_eq!(c2, c_lookup2);
        }

        #[tokio::test]
        async fn get_or_add_corpus() {
            let db = Database::memory().await.unwrap();
            let c1 = db.get_or_add_corpus("media").await.unwrap();
            let c2 = db.get_or_add_corpus("media2").await.unwrap();
            let c1_2 = db.get_or_add_corpus("media").await.unwrap();
            let c2_2 = db.get_or_add_corpus("media2").await.unwrap();
            assert_eq!(c1, c1_2);
            assert_eq!(c2, c2_2);
        }

        #[tokio::test]
        async fn list_all_corpus() {
            let db = Database::memory().await.unwrap();
            assert_eq!(db.list_corpus().await.unwrap(), vec![]);
            let c1 = db.get_or_add_corpus("media").await.unwrap();
            assert_eq!(db.list_corpus().await.unwrap(), vec![c1.clone()]);
            let c2 = db.get_or_add_corpus("media2").await.unwrap();
            assert_eq!(db.list_corpus().await.unwrap(), vec![c1, c2]);
        }
    }

    mod chapter {
        use lucile_core::{identifiers::ChapterId, metadata::MediaHash};

        use super::*;

        #[tokio::test]
        async fn define_chapter() {
            let db = Database::memory().await.unwrap();
            let c = db.add_corpus("media").await.unwrap();
            let id = db
                .define_chapter(
                    c.id.unwrap(),
                    "title",
                    Some(1),
                    Some(2),
                    MediaHash::from_bytes(b"data"),
                )
                .await
                .unwrap();
            assert_eq!(id, ChapterId::new(1))
        }

        #[tokio::test]
        async fn define_chapter_without_episodes() {
            let db = Database::memory().await.unwrap();
            let c = db.add_corpus("media").await.unwrap();
            let id = db
                .define_chapter(
                    c.id.unwrap(),
                    "title",
                    None,
                    None,
                    MediaHash::from_bytes(b"data"),
                )
                .await
                .unwrap();
            assert_eq!(id, ChapterId::new(1))
        }

        #[tokio::test]
        async fn define_chapter_without_title() {
            let db = Database::memory().await.unwrap();
            let c = db.add_corpus("media").await.unwrap();
            assert_err_is_constraint(
                db.define_chapter(
                    c.id.unwrap(),
                    "",
                    None,
                    None,
                    MediaHash::from_bytes(b"data"),
                )
                .await,
                "CHECK",
            )
        }

        #[tokio::test]
        async fn define_and_update_chapter() {
            let db = Database::memory().await.unwrap();
            let c = db.add_corpus("media").await.unwrap();
            let id1 = db
                .define_chapter(
                    c.id.unwrap(),
                    "title1",
                    Some(1),
                    Some(1),
                    MediaHash::from_bytes(b"data"),
                )
                .await
                .unwrap();
            assert_eq!(id1, ChapterId::new(1));

            let id2 = db
                .define_chapter(
                    c.id.unwrap(),
                    "title2",
                    Some(1),
                    Some(2),
                    MediaHash::from_bytes(b"data2"),
                )
                .await
                .unwrap();
            assert_eq!(id2, ChapterId::new(2));

            let idx = db
                .define_chapter(
                    c.id.unwrap(),
                    "title_updated",
                    Some(1),
                    Some(1),
                    MediaHash::from_bytes(b"data"),
                )
                .await
                .unwrap();
            assert_eq!(idx, id1);
        }

        #[tokio::test]
        async fn define_and_redefine_chapter() {
            let db = Database::memory().await.unwrap();
            let c = db.add_corpus("media").await.unwrap();
            let id1 = db
                .define_chapter(
                    c.id.unwrap(),
                    "title1",
                    Some(1),
                    Some(1),
                    MediaHash::from_bytes(b"data"),
                )
                .await
                .unwrap();
            assert_eq!(id1, ChapterId::new(1));

            let id2 = db
                .define_chapter(
                    c.id.unwrap(),
                    "title1",
                    Some(1),
                    Some(1),
                    MediaHash::from_bytes(b"data"),
                )
                .await
                .unwrap();
            assert_eq!(id1, id2);
        }
    }
    mod media {
        use std::time::Duration;

        use lucile_core::{identifiers::MediaViewId, metadata::MediaHash};

        use super::*;

        #[tokio::test]
        async fn define_media_view() {
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
            let id = db.add_media_view(ch_id, "test-view").await.unwrap();
            assert_eq!(id, MediaViewId::new(1))
        }

        #[tokio::test]
        async fn define_nameless_media_view() {
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
            assert_err_is_constraint(db.add_media_view(ch_id, "").await, "CHECK");
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
                media_view_id,
                MediaHash::from_bytes(b"s1data"),
                Duration::from_secs_f64(1.2),
                Duration::from_secs_f64(10.3),
                None,
            )
            .await
            .unwrap();
            db.add_media_segment(
                media_view_id,
                MediaHash::from_bytes(b"s2data"),
                Duration::from_secs_f64(10.3),
                Duration::from_secs_f64(14.1),
                Some("foo".to_string()),
            )
            .await
            .unwrap();
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
                    media_view_id,
                    MediaHash::from_bytes(b"s1data"),
                    Duration::from_secs_f64(1.2),
                    Duration::from_secs_f64(10.3),
                    Some(String::new()),
                )
                .await,
                "CHECK",
            )
        }

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

    mod subtitle {
        use lucile_core::{metadata::MediaHash, Subtitle};

        use super::*;

        const SUB1: &str = include_str!("../test_data/srt1.srt");
        const SUB2: &str = include_str!("../test_data/srt2.srt");

        fn parse_subs(srt: &str) -> Vec<Subtitle> {
            subrip::parse(srt).expect("test SRT failed to parse")
        }

        #[tokio::test]
        async fn add_subs() {
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

            let s1 = parse_subs(SUB1);
            let u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
        }

        #[tokio::test]
        async fn update_subs() {
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

            let s1 = parse_subs(SUB1);
            let s2 = parse_subs(SUB2);
            let u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
            let u2 = db.add_subtitles(ch_id, &s2).await.unwrap();
            assert_ne!(u1, u2)
        }

        #[tokio::test]
        async fn dup_subs_in_different_media() {
            let db = Database::memory().await.unwrap();
            let corpus = db.add_corpus("media").await.unwrap();
            let ch_id1 = db
                .define_chapter(
                    corpus.id.unwrap(),
                    "c1",
                    None,
                    None,
                    MediaHash::from_bytes(b"data"),
                )
                .await
                .unwrap();
            let ch_id2 = db
                .define_chapter(
                    corpus.id.unwrap(),
                    "c1",
                    None,
                    None,
                    MediaHash::from_bytes(b"data2"),
                )
                .await
                .unwrap();

            let s1 = parse_subs(SUB1);
            let u1 = db.add_subtitles(ch_id1, &s1).await.unwrap();
            let u2 = db.add_subtitles(ch_id2, &s1).await.unwrap();
            assert_ne!(u1, u2)
        }

        #[tokio::test]
        async fn dup_subs_in_same_media() {
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

            let s1 = parse_subs(SUB1);
            let u1 = db.add_subtitles(ch_id, &s1).await.unwrap();
            let u2 = db.add_subtitles(ch_id, &s1).await.unwrap();
            // TODO this is the desired behavior!
            // assert_eq!(u1, u2)
        }
    }
}

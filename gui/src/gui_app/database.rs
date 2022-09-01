use anyhow::Context;
pub use corpus_type::CorpusType;
use rusqlite::{params, Connection};
use serde::{de::DeserializeOwned, Serialize};
use std::{borrow::Cow, path::Path};

const DB_PATH_NAME: &str = "sift.db";

pub struct AppData {
    conn: Connection,
}

pub trait ItemDB: Serialize + DeserializeOwned {
    fn name(&'_ self) -> Cow<'_, str>;
}

impl ItemDB for usize {
    fn name(&'_ self) -> Cow<'_, str> {
        Cow::from(format!("{}", self))
    }
}

impl AppData {
    pub fn open(root_dir: &Path) -> anyhow::Result<AppData> {
        let p = root_dir.join(DB_PATH_NAME);
        std::fs::create_dir_all(root_dir)
            .with_context(|| format!("could not create dir: {:?}", root_dir))?;
        let conn = Connection::open(p.as_path())
            .with_context(|| format!("could not open sqlite db at `{:?}`", p))?;
        Ok(AppData { conn })
    }

    // pub fn memory() -> anyhow::Result<AppData> {
    //     let conn = Connection::open_in_memory().context("unable to open in memory sqlite db")?;
    //     Ok(AppData { conn })
    // }

    pub fn init(&self) -> anyhow::Result<()> {
        create_tables(&self.conn).context("could not create tables")
    }

    pub fn get_corpuses(&mut self) -> anyhow::Result<Vec<Corpus>> {
        let tx = self.conn.transaction()?;
        let mut items = Vec::new();
        {
            let mut stmt = tx.prepare("SELECT id, name, type FROM corpus")?;

            let corpus_iter = stmt.query_map([], |row| {
                Ok(Corpus {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    corpus_type: row.get(2)?,
                })
            })?;

            for c in corpus_iter {
                let c = c?;
                items.push(c);
            }
        }

        tx.commit()?;

        Ok(items)
    }

    pub fn new_corpus<S: Into<String>>(
        &self,
        name: S,
        corpus_type: CorpusType,
    ) -> anyhow::Result<Corpus> {
        let name = name.into();
        let mut stmt = self
            .conn
            .prepare("INSERT INTO corpus (name, type) VALUES (?1, ?2)")?;
        let id = stmt.insert(params![name.as_str(), corpus_type])?;

        Ok(Corpus {
            id: id as u32,
            name,
            corpus_type,
        })
    }

    pub fn add_items<T: ItemDB>(
        &mut self,
        corpus: &Corpus,
        items: &[T],
    ) -> anyhow::Result<Vec<u32>> {
        let tx = self.conn.transaction()?;
        let mut ids = Vec::with_capacity(items.len());
        {
            let mut stmt = tx.prepare(
                "INSERT INTO item (name, corpus_id, ignored, data)
                    VALUES (?1, ?2, ?3, ?4)",
            )?;

            for item in items {
                let name = item.name();
                let value = serde_json::to_value(&item)?;
                let id = stmt.insert(params![name, corpus.id, false, &value])?;
                ids.push(id as u32);
            }
        }

        tx.commit()?;

        Ok(ids)
    }

    pub fn get_items<T: ItemDB>(&mut self, corpus: &Corpus) -> anyhow::Result<Vec<Item<T>>> {
        let tx = self.conn.transaction()?;
        let mut items = Vec::new();
        {
            let mut stmt =
                tx.prepare("SELECT id, data FROM item WHERE ignored=0 AND corpus_id = (?1)")?;

            let item_iter = stmt.query_map([corpus.id], |row| {
                let data: serde_json::Value = row.get(1)?;
                Ok(Item {
                    id: row.get(0)?,
                    ignored: false,
                    inner: data,
                })
            })?;

            for item in item_iter {
                let item = item?;
                items.push(Item {
                    id: item.id,
                    ignored: item.ignored,
                    inner: serde_json::from_value(item.inner)?,
                });
            }
        }

        tx.commit()?;

        Ok(items)
    }

    pub fn save_preference(
        &self,
        lhs: u32,
        rhs: u32,
        outcome: ranker::Outcome,
    ) -> anyhow::Result<()> {
        let time = chrono::prelude::Utc::now();
        let outcome = outcome_type::OutcomeType::from(outcome);
        let mut stmt = self.conn.prepare(
            "INSERT INTO preference (time, lhs_id, rhs_id, outcome) VALUES (?1, ?2, ?3, ?4)",
        )?;
        stmt.insert(params![time, lhs, rhs, outcome])?;

        Ok(())
    }

    pub fn load_preferences(
        &mut self,
        corpus: &Corpus,
        mut f: impl FnMut(Preference) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let iter = {
            let mut stmt = self.conn.prepare(
                "SELECT
                    p.time, p.lhs_id, p.rhs_id, p.outcome
                FROM preference p
                JOIN item l ON p.lhs_id = l.id
                WHERE l.corpus_id = (?1)", // "SELECT time, lhs_id, rhs_id, outcome FROM preference WHERE corpus_id = (?1)",
            )?;

            let iter = stmt.query_map([corpus.id], |row| {
                let outcome_type: outcome_type::OutcomeType = row.get(3)?;
                Ok(Preference {
                    // time: row.get(0)?,
                    lhs_id: row.get(1)?,
                    rhs_id: row.get(2)?,
                    outcome: outcome_type.0,
                })
            })?;

            for item in iter {
                let item = item?;
                f(item)?;
            }
        };

        Ok(iter)
    }
}

mod outcome_type {
    use ranker::Outcome;
    use rusqlite::types::{FromSql, FromSqlError, ToSql};

    impl From<Outcome> for OutcomeType {
        fn from(o: Outcome) -> Self {
            OutcomeType(o)
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct OutcomeType(pub(crate) Outcome);

    impl ToSql for OutcomeType {
        fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
            Ok(match self.0 {
                Outcome::Left => 0.into(),
                Outcome::Right => 1.into(),
                Outcome::Equal => 2.into(),
            })
        }
    }

    impl FromSql for OutcomeType {
        fn column_result(
            value: rusqlite::types::ValueRef<'_>,
        ) -> rusqlite::types::FromSqlResult<Self> {
            i64::column_result(value).and_then(|as_i64| match as_i64 {
                0 => Ok(Outcome::Left.into()),
                1 => Ok(Outcome::Right.into()),
                2 => Ok(Outcome::Equal.into()),
                _ => Err(FromSqlError::InvalidType),
            })
        }
    }
}
mod corpus_type {
    use rusqlite::types::{FromSql, FromSqlError, ToSql};

    #[derive(Debug, Clone, Copy)]
    pub enum CorpusType {
        Numeric,
        ReferenceImage,
    }

    impl ToSql for CorpusType {
        fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
            Ok(match self {
                CorpusType::Numeric => 0.into(),
                CorpusType::ReferenceImage => 1.into(),
            })
        }
    }

    impl FromSql for CorpusType {
        fn column_result(
            value: rusqlite::types::ValueRef<'_>,
        ) -> rusqlite::types::FromSqlResult<Self> {
            i64::column_result(value).and_then(|as_i64| match as_i64 {
                0 => Ok(CorpusType::Numeric),
                1 => Ok(CorpusType::ReferenceImage),
                _ => Err(FromSqlError::InvalidType),
            })
        }
    }
}

#[derive(Debug)]
pub struct Item<T> {
    pub(crate) id: u32,
    ignored: bool,
    pub(crate) inner: T,
}

impl<T> Item<T> {
    pub(crate) fn map_result<V>(
        self,
        f: impl FnOnce(T) -> anyhow::Result<V>,
    ) -> anyhow::Result<Item<V>> {
        let Item { id, ignored, inner } = self;
        let inner = f(inner)?;
        Ok(Item { id, ignored, inner })
    }
}

#[derive(Debug, Clone)]
pub struct Corpus {
    pub id: u32,
    pub name: String,
    pub corpus_type: CorpusType,
}

pub struct Preference {
    // pub(crate) time: chrono::DateTime<chrono::Utc>,
    pub(crate) lhs_id: u32,
    pub(crate) rhs_id: u32,
    pub(crate) outcome: ranker::Outcome,
}

fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS corpus (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            type INTEGER NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS item (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            corpus_id INTEGER NOT NULL,
            ignored INTEGER NOT NULL,
            data BLOB,
            FOREIGN KEY(corpus_id) REFERENCES corpus(id),
            UNIQUE(corpus_id,name)
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS preference (
            time TEXT NOT NULL,
            lhs_id INTEGER NOT NULL,
            rhs_id INTEGER NOT NULL,
            outcome INTEGER NOT NULL,
            FOREIGN KEY(lhs_id) REFERENCES item(id),
            FOREIGN KEY(rhs_id) REFERENCES item(id)
        )",
        [],
    )?;

    Ok(())
}

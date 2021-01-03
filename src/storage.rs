use crate::{
    content::{Content, FileSystemContent},
    error::TError,
    srt_loader::IndexableEpisode,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader, path};
use tantivy::Index;

const INDEX_DIR: &str = "index";
const DB_JSON: &str = "db.json";

pub struct Storage {
    // pub content: Content,
    // pub videos: FileSystemContent,
    storage_path: path::PathBuf,
    // pub index: Index,
}

#[derive(Serialize, Deserialize)]
pub struct Database {
    pub id: uuid::Uuid,
    pub content: Content,
    pub videos: FileSystemContent,
}

impl Storage {
    fn write_db(
        &self,
        content: Content,
        videos: FileSystemContent,
        id: uuid::Uuid,
    ) -> anyhow::Result<()> {
        let database = Database {
            id,
            content,
            videos,
        };
        let db_path = self.storage_path.join(DB_JSON);
        std::fs::create_dir_all(self.storage_path.as_path())?;
        let mut f = File::create(&db_path)?;
        serde_json::to_writer_pretty(&mut f, &database)?;
        Ok(())
    }
    pub fn new<P: AsRef<path::Path>>(storage: P) -> Storage {
        let storage_path = storage.as_ref();
        Storage {
            storage_path: storage_path.to_owned(),
        }
    }
    pub fn load(&self) -> Result<Database> {
        let db_path = self.storage_path.join(DB_JSON);
        let mut f = BufReader::new(File::open(&db_path)?);
        log::trace!("loading data from: {:?}", db_path.as_path());
        let db: Database = serde_json::from_reader(&mut f)?;
        Ok(db)
    }
    pub fn index(&self) -> Result<Index> {
        let index_dir = self.storage_path.join(INDEX_DIR);
        log::trace!("loading search index from: {:?}", index_dir.as_path());
        let index = Index::open_in_dir(index_dir.as_path())
            .map_err(|e| TError::from(e))
            .with_context(|| format!("unable to open index from: {:?}", index_dir))?;
        Ok(index)
    }
    pub fn build_index(
        &self,
        content: Content,
        videos: FileSystemContent,
        max_window: usize,
    ) -> anyhow::Result<Index> {
        let index_path = self.storage_path.join(INDEX_DIR);
        let indexable_episodes = content
            .episodes
            .iter()
            .map(|e| IndexableEpisode::from(e.clone()))
            .collect::<Vec<_>>();
        std::fs::create_dir_all(index_path.as_path())?;
        let index = crate::search::build_index(&index_path, &indexable_episodes, max_window)
            .map_err(|e| TError::from(e))?;
        let unique_id = uuid::Uuid::new_v4();
        self.write_db(content, videos, unique_id)?;
        Ok(index)
    }
}

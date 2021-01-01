use crate::{error::TError, srt_loader::Episode};
use anyhow::Context;
use std::{fs::File, path};
use tantivy::{Index, TantivyError};

const INDEX_DIR: &str = "index";
const EPISODES_JSON: &str = "episodes.json";

pub struct Storage {
    pub episodes: Vec<Episode>,
    storage_path: path::PathBuf,
    pub index: Index,
}

impl Storage {
    pub fn save_episodes(&self) -> anyhow::Result<()> {
        let p = self.storage_path.join(EPISODES_JSON);
        std::fs::create_dir_all(self.storage_path.as_path())?;
        let mut f = File::create(&p)?;
        serde_json::to_writer_pretty(&mut f, &self.episodes)?;
        Ok(())
    }
    pub fn load<P: AsRef<path::Path>>(storage: P) -> anyhow::Result<Storage> {
        let storage_path = storage.as_ref();
        let p = storage_path.join(EPISODES_JSON);
        let mut f = File::open(&p)?;
        log::trace!("loading episode data from: {:?}", p.as_path());
        let episodes: Vec<Episode> = serde_json::from_reader(&mut f)?;
        let index_dir = storage_path.join(INDEX_DIR);
        log::trace!("loading search index from: {:?}", index_dir.as_path());
        let index = Index::open_in_dir(index_dir.as_path())
            .map_err(|e| TError::from(e))
            .with_context(|| format!("unable to open index from: {:?}", index_dir))?;
        log::trace!("successfully loaded data from disk");
        Ok(Storage {
            episodes,
            storage_path: storage_path.to_owned(),
            index,
        })
    }
    pub fn build_index<P: AsRef<path::Path>>(
        storage: P,
        eps: Vec<Episode>,
        max_window: usize,
    ) -> anyhow::Result<Storage> {
        let storage_path = storage.as_ref();
        let index_path = storage_path.join(INDEX_DIR);
        std::fs::create_dir_all(index_path.as_path())?;
        let index = crate::search::build_index(&index_path, eps.as_slice(), max_window)
            .map_err(|e| TError::from(e))?;
        let s = Storage {
            episodes: eps,
            storage_path: storage_path.to_owned(),
            index,
        };
        s.save_episodes()?;
        Ok(s)
    }
}

use crate::{
    content::{Content, Episode, FileSystemContent},
    details::{ContentData, MediaHash, SegmentedVideo},
    error::TError,
    srt_loader::IndexableEpisode,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, io::BufReader, path};
use tantivy::Index;

const INDEX_DIR: &str = "index";
const CONTENT_DB_JSON: &str = "content.json";

pub struct Storage {
    storage_path: path::PathBuf,
    index_name: String,
    index_path: path::PathBuf,
    episodes: path::PathBuf,
    segments: path::PathBuf,
    media: path::PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct ContentDatabase {
    pub name: String,
    pub original_files: HashMap<MediaHash, path::PathBuf>,
}

impl Storage {
    pub fn prepare(&self) -> Result<()> {
        std::fs::create_dir_all(self.storage_path.as_path())
            .with_context(|| format!("unable to write {:?}", self.storage_path))?;
        std::fs::create_dir_all(self.episodes.as_path())
            .with_context(|| format!("unable to write {:?}", self.episodes))?;
        std::fs::create_dir_all(self.segments.as_path())
            .with_context(|| format!("unable to write {:?}", self.segments))?;
        std::fs::create_dir_all(self.media.as_path())
            .with_context(|| format!("unable to write {:?}", self.media))?;
        Ok(())
    }

    pub fn new<P: AsRef<path::Path>, S: Into<String>>(storage: P, index: S) -> Storage {
        let storage_path = storage.as_ref();
        let index_name = index.into();
        let index_path = storage_path.join(index_name.as_str());
        Storage {
            storage_path: storage_path.to_owned(),
            episodes: index_path.join("episodes"),
            media: storage_path.join("media"),
            segments: storage_path.join("segments"),
            index_name,
            index_path,
        }
    }
    pub fn index(&self) -> Result<Index> {
        let index_dir = self.storage_path.join(INDEX_DIR);
        log::trace!("loading search index from: {:?}", index_dir.as_path());
        let index = Index::open_in_dir(index_dir.as_path())
            .map_err(|e| TError::from(e))
            .with_context(|| format!("unable to open index from: {:?}", index_dir))?;
        Ok(index)
    }

    pub fn write_content_db(
        &self,
        name: String,
        media: HashMap<MediaHash, path::PathBuf>,
    ) -> Result<()> {
        let db_path = self.index_path.join(CONTENT_DB_JSON);
        let mut f =
            File::create(&db_path).with_context(|| format!("unable to write {:?}", db_path))?;
        serde_json::to_writer_pretty(
            &mut f,
            &ContentDatabase {
                name,
                original_files: media,
            },
        )?;
        Ok(())
    }

    pub fn write_content(&self, media_id: MediaHash, e: &ContentData) -> Result<()> {
        let e_path = self.episodes.join(media_id.to_string());
        let mut f =
            File::create(&e_path).with_context(|| format!("unable to write {:?}", e_path))?;
        serde_json::to_writer_pretty(&mut f, &e)?;
        Ok(())
    }

    pub fn load_content(&self, media_id: &MediaHash) -> Result<ContentData> {
        let e_path = self.episodes.join(media_id.to_string());
        let mut f = File::open(&e_path).with_context(|| format!("unable to open {:?}", e_path))?;
        let e: ContentData = serde_json::from_reader(&mut f)?;
        Ok(e)
    }
    pub fn load_content_db(&self) -> Result<ContentDatabase> {
        let db_path = self.index_path.join(CONTENT_DB_JSON);
        let mut f =
            File::open(&db_path).with_context(|| format!("unable to open {:?}", db_path))?;
        let db: ContentDatabase = serde_json::from_reader(&mut f)?;
        Ok(db)
    }

    pub fn save_media_map(&self, media_id: &MediaHash, video: &SegmentedVideo) -> Result<()> {
        let e_path = self.segments.join(media_id.to_string());
        let mut f =
            File::create(&e_path).with_context(|| format!("unable to write {:?}", e_path))?;
        serde_json::to_writer_pretty(&mut f, video)?;
        Ok(())
    }

    pub fn load_media_map(&self, media_id: &MediaHash) -> Result<SegmentedVideo> {
        let e_path = self.episodes.join(media_id.to_string());
        let mut f = File::open(&e_path).with_context(|| format!("unable to open {:?}", e_path))?;
        let video: SegmentedVideo = serde_json::from_reader(&mut f)?;
        Ok(video)
    }

    pub fn build_index(&self, max_window: usize) -> anyhow::Result<Index> {
        let db = self.load_content_db()?;
        let index_path = self.index_path.join(INDEX_DIR);
        let indexable_episodes = db
            .original_files
            .keys()
            .map(|e| self.load_content(e).map(|c| IndexableEpisode::from(c)))
            .collect::<Result<Vec<_>>>()?;
        let _ = std::fs::remove_dir_all(index_path.as_path());
        std::fs::create_dir_all(index_path.as_path())
            .with_context(|| format!("could not create {:?}", index_path))?;
        let index = crate::search::build_index(&index_path, &indexable_episodes, max_window)
            .map_err(|e| TError::from(e))?;
        Ok(index)
    }

    pub fn storage_path(&self) -> &path::Path {
        self.media.as_path()
    }
}

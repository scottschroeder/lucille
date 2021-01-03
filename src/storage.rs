use crate::{
    content::{Content, FileSystemContent},
    error::TError,
    srt_loader::IndexableEpisode,
};
use anyhow::{Context, Result};
use std::{fs::File, io::BufReader, path};
use tantivy::Index;

const INDEX_DIR: &str = "index";
const EPISODES_JSON: &str = "content.json";
const VIDEOS_JSON: &str = "videos.json";

pub struct Storage {
    // pub content: Content,
    // pub videos: FileSystemContent,
    storage_path: path::PathBuf,
    // pub index: Index,
}

impl Storage {
    pub fn save_episodes(
        &self,
        content: &Content,
        videos: &FileSystemContent,
    ) -> anyhow::Result<()> {
        let episode_path = self.storage_path.join(EPISODES_JSON);
        let video_path = self.storage_path.join(VIDEOS_JSON);
        std::fs::create_dir_all(self.storage_path.as_path())?;
        let mut f_content = File::create(&episode_path)?;
        let mut f_video = File::create(&video_path)?;
        serde_json::to_writer_pretty(&mut f_content, &content)?;
        serde_json::to_writer_pretty(&mut f_video, &videos)?;
        Ok(())
    }
    pub fn new<P: AsRef<path::Path>>(storage: P) -> Storage {
        let storage_path = storage.as_ref();
        Storage {
            storage_path: storage_path.to_owned(),
        }
    }
    pub fn content(&self) -> Result<Content> {
        let content_path = self.storage_path.join(EPISODES_JSON);
        let mut f_content = BufReader::new(File::open(&content_path)?);
        log::trace!("loading episode data from: {:?}", content_path.as_path());
        let content: Content = serde_json::from_reader(&mut f_content)?;
        Ok(content)
    }
    pub fn videos(&self) -> Result<FileSystemContent> {
        let video_path = self.storage_path.join(VIDEOS_JSON);
        let mut f_video = BufReader::new(File::open(&video_path)?);
        log::trace!("loading video data from: {:?}", video_path.as_path());
        let videos: FileSystemContent = serde_json::from_reader(&mut f_video)?;
        Ok(videos)
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
        self.save_episodes(&content, &videos)?;
        Ok(index)
    }
}

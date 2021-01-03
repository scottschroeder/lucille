use crate::{
    content::{Content, FileSystemContent},
    error::TError,
    srt_loader::IndexableEpisode,
};
use anyhow::Context;
use std::{fs::File, path};
use tantivy::Index;

const INDEX_DIR: &str = "index";
const EPISODES_JSON: &str = "content.json";
const VIDEOS_JSON: &str = "videos.json";

pub struct Storage {
    pub content: Content,
    pub videos: FileSystemContent,
    storage_path: path::PathBuf,
    pub index: Index,
}

impl Storage {
    pub fn save_episodes(&self) -> anyhow::Result<()> {
        let episode_path = self.storage_path.join(EPISODES_JSON);
        let video_path = self.storage_path.join(VIDEOS_JSON);
        std::fs::create_dir_all(self.storage_path.as_path())?;
        let mut f_content = File::create(&episode_path)?;
        let mut f_video = File::create(&video_path)?;
        serde_json::to_writer_pretty(&mut f_content, &self.content)?;
        serde_json::to_writer_pretty(&mut f_video, &self.videos)?;
        Ok(())
    }
    pub fn load<P: AsRef<path::Path>>(storage: P) -> anyhow::Result<Storage> {
        let storage_path = storage.as_ref();
        let content_path = storage_path.join(EPISODES_JSON);
        let video_path = storage_path.join(VIDEOS_JSON);
        let mut f_content = File::open(&content_path)?;
        let mut f_video = File::open(&video_path)?;
        log::trace!("loading episode data from: {:?}", content_path.as_path());
        let content: Content = serde_json::from_reader(&mut f_content)?;
        log::trace!("loading video data from: {:?}", video_path.as_path());
        let videos: FileSystemContent = serde_json::from_reader(&mut f_video)?;
        let index_dir = storage_path.join(INDEX_DIR);
        log::trace!("loading search index from: {:?}", index_dir.as_path());
        let index = Index::open_in_dir(index_dir.as_path())
            .map_err(|e| TError::from(e))
            .with_context(|| format!("unable to open index from: {:?}", index_dir))?;
        log::trace!("successfully loaded data from disk");
        Ok(Storage {
            content,
            videos,
            storage_path: storage_path.to_owned(),
            index,
        })
    }
    pub fn build_index<P: AsRef<path::Path>>(
        storage: P,
        content: Content,
        videos: FileSystemContent,
        max_window: usize,
    ) -> anyhow::Result<Storage> {
        let storage_path = storage.as_ref();
        let index_path = storage_path.join(INDEX_DIR);
        let indexable_episodes = content
            .episodes
            .iter()
            .map(|e| IndexableEpisode::from(e.clone()))
            .collect::<Vec<_>>();
        std::fs::create_dir_all(index_path.as_path())?;
        let index = crate::search::build_index(&index_path, &indexable_episodes, max_window)
            .map_err(|e| TError::from(e))?;
        let s = Storage {
            content,
            videos,
            storage_path: storage_path.to_owned(),
            index,
        };
        s.save_episodes()?;
        Ok(s)
    }
}

use std::sync::Arc;

use dashmap::DashMap;
use lucile_core::{metadata::MediaMetadata, Subtitle};

pub struct EpisodeData {
    pub metadata: MediaMetadata,
    pub subs: Vec<Subtitle>,
}

#[derive(Clone, Default)]
pub struct EpisodeCache {
    pub inner: Arc<DashMap<i64, EpisodeData>>,
}

impl EpisodeCache {
    pub fn contains(&self, id: i64) -> bool {
        self.inner.contains_key(&id)
    }

    pub fn insert(&self, id: i64, data: EpisodeData) {
        self.inner.insert(id, data);
    }

    pub fn episode(&self, id: i64) -> dashmap::mapref::one::Ref<'_, i64, EpisodeData> {
        self.inner
            .get(&id)
            .expect("key should be inserted before lookup")
    }
}

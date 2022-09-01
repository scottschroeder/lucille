use super::{
    database::{AppData, Corpus, Item, ItemDB},
    sift_app::SiftApp,
    siftimage::SiftImage,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEntry {
    pub(crate) name: String,
    pub(crate) full_path: String,
}

impl ItemDB for ImageEntry {
    fn name(&'_ self) -> Cow<'_, str> {
        Cow::from(self.full_path.as_str())
    }
}

pub struct NamedImage {
    pub entry: ImageEntry,
    pub(crate) image: SiftImage,
}

impl ImageEntry {
    fn load(self) -> anyhow::Result<NamedImage> {
        let image = SiftImage::from_path(&self.name, Path::new(&self.full_path))
            .with_context(|| format!("could not load image `{:?}`", self.full_path))?;
        Ok(NamedImage { entry: self, image })
    }
}

pub fn update_db_from_directory<P: AsRef<Path>>(
    db: &mut AppData,
    corpus: &Corpus,
    p: P,
) -> anyhow::Result<()> {
    let p = p.as_ref();
    let mut entries = Vec::new();
    for d in std::fs::read_dir(p)? {
        let d = d?;
        let path = d.path();

        if let Some(path_string) = path.to_str() {
            let short_name = path.file_stem().and_then(|n| n.to_str()).ok_or_else(|| {
                anyhow::anyhow!("could not determine filename from path: {:?}", path)
            })?;
            entries.push(ImageEntry {
                name: short_name.to_string(),
                full_path: path_string.to_string(),
            });
        }
    }

    let _ids = db.add_items(corpus, &entries)?;
    Ok(())
}

pub(crate) fn load_app_from_db_realz(
    db: &mut AppData,
    corpus: Corpus,
) -> anyhow::Result<SiftApp<NamedImage>> {
    let items: Vec<Item<ImageEntry>> = db.get_items(&corpus)?;
    // let mut images = Vec::with_capacity(items.len());

    let images = items
        .into_iter()
        .map(|e| e.map_result(|ie| ie.load()))
        .filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                log::warn!("{}", e);
                None
            }
        })
        .collect::<Vec<_>>();

    SiftApp::new(corpus, images)
}

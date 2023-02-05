use std::path::{Path, PathBuf};

use database::{Database, DatabaseError};
use lucile_core::uuid::Uuid;
use search::SearchIndex;

use crate::{search_manager::SearchService, LucileAppError};

const QUALIFIER: &str = "io";
const ORGANIZATION: &str = "vauntware";
const APP: &str = "lucile";

const INDEX_DIR: &str = "index";
const DEFAULT_DB_NAME: &str = "lucile.db";

#[derive(Debug)]
pub struct LucileApp {
    pub db: Database,
    pub dirs: directories::ProjectDirs,
    index_root_override: Option<PathBuf>,
}

async fn load_db_from_env() -> Result<Option<Database>, DatabaseError> {
    match Database::from_env().await {
        Ok(db) => Ok(Some(db)),
        Err(e) => match e {
            DatabaseError::NoDatabaseSpecified => Ok(None),
            _ => Err(e),
        },
    }
}

fn project_dirs() -> directories::ProjectDirs {
    directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APP).unwrap()
}

impl LucileApp {
    pub async fn create<PR: AsRef<Path>, P: Into<PathBuf>>(
        database_path: Option<PR>,
        index_path: Option<P>,
    ) -> Result<Self, LucileAppError> {
        let dirs = project_dirs();

        let db = if let Some(url) = database_path {
            Database::from_path(url).await?
        } else {
            match load_db_from_env().await? {
                Some(db) => db,
                None => {
                    let db_path = dirs.data_dir().join(DEFAULT_DB_NAME);
                    Database::from_path(db_path).await?
                }
            }
        };

        let index_root = index_path.map(|p| p.into());

        Ok(LucileApp {
            db,
            dirs,
            index_root_override: index_root,
        })
    }

    pub fn search_service(&self, index_uuid: Uuid) -> Result<SearchService, LucileAppError> {
        let index_dir = self.index_root().join(index_uuid.to_string());
        log::trace!("loading search index from: {:?}", index_dir.as_path());
        let index = SearchIndex::open_in_dir(index_uuid, index_dir)?;
        Ok(SearchService { index })
    }
}

impl LucileApp {
    pub fn index_root(&self) -> PathBuf {
        self.index_root_override
            .as_ref()
            .cloned()
            .unwrap_or_else(|| self.dirs.data_dir().join(INDEX_DIR))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub struct LucileTestApp {
        pub app: LucileApp,
        pub dir: tempfile::TempDir,
    }

    pub async fn lucile_test_app() -> LucileTestApp {
        let dir = tempfile::TempDir::new().expect("unable to create tmpdir");
        let app = LucileApp {
            db: Database::memory()
                .await
                .expect("could not build in memory database"),
            dirs: project_dirs(),
            index_root_override: Some(dir.path().join("index_root")),
        };
        LucileTestApp { app, dir }
    }
}

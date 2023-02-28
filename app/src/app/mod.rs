use std::path::Path;

use database::Database;
use lucile_core::uuid::Uuid;
use search::SearchIndex;

use crate::{search_manager::SearchService, LucileAppError};

mod lucile_config;

pub use lucile_config::{ConfigBuilder, ConfigError, LucileConfig};

#[derive(Debug)]
pub struct LucileBuilder {
    pub config: lucile_config::ConfigBuilder,
}

impl LucileBuilder {
    pub fn new() -> Result<Self, ConfigError> {
        Ok(LucileBuilder {
            config: lucile_config::ConfigBuilder::new()?.load_environment(true),
        })
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn config_file(self, config_file: Option<&Path>) -> Result<Self, ConfigError> {
        self.update(|c| c.config_file(config_file))
    }

    fn update(
        self,
        f: impl FnOnce(
            lucile_config::ConfigBuilder,
        ) -> Result<lucile_config::ConfigBuilder, ConfigError>,
    ) -> Result<Self, ConfigError> {
        let LucileBuilder {
            config: config_builder,
        } = self;
        let config_builder = f(config_builder)?;
        Ok(Self {
            config: config_builder,
        })
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn index_root(self, index_root: Option<&Path>) -> Result<Self, ConfigError> {
        self.update(|c| c.index_root(index_root))
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn database_path(self, database_path: Option<&Path>) -> Result<Self, ConfigError> {
        self.update(|c| c.database_path(database_path))
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn media_root(self, media_root: Option<&Path>) -> Result<Self, ConfigError> {
        self.update(|c| c.media_root(media_root))
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn ffmpeg_override(self, ffmpeg: Option<&Path>) -> Result<Self, ConfigError> {
        self.update(|c| c.ffmpeg_override(ffmpeg))
    }

    pub async fn build(self) -> Result<LucileApp, LucileAppError> {
        let Self {
            config: config_builder,
        } = self;
        let config = config_builder.build()?;

        let db_opts = config.database_connection_opts()?;
        let mut db_builder = database::DatabaseBuider::default();
        db_builder.add_opts(db_opts)?;
        db_builder.connect().await?;
        db_builder.migrate().await?;
        let (db, _) = db_builder.into_parts()?;

        let app = LucileApp { db, config };
        log::trace!("{:#?}", app);
        Ok(app)
    }
}

#[derive(Debug)]
pub struct LucileApp {
    pub db: Database,
    pub config: lucile_config::LucileConfig,
}

impl LucileApp {
    pub fn search_service(&self, index_uuid: Uuid) -> Result<SearchService, LucileAppError> {
        let index_dir = self.config.index_root().join(index_uuid.to_string());
        log::debug!("loading search index from: {:?}", index_dir.as_path());
        let index = SearchIndex::open_in_dir(index_uuid, index_dir)?;
        Ok(SearchService { index })
    }
}

#[cfg(test)]
pub mod tests {
    use lucile_config::ConfigBuilder;

    use super::*;

    pub struct LucileTestApp {
        pub app: LucileApp,
        pub dir: tempfile::TempDir,
    }

    pub async fn lucile_test_app() -> LucileTestApp {
        let dir = tempfile::TempDir::new().expect("unable to create tmpdir");
        let db = Database::memory()
            .await
            .expect("could not build in memory database");

        let test_config =
            ConfigBuilder::new_test_config(dir.path()).expect("could not create test config");

        let app = LucileApp {
            db,
            config: test_config,
        };
        LucileTestApp { app, dir }
    }
}

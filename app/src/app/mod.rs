use std::path::Path;

use database::Database;
use lucille_core::uuid::Uuid;
use search::SearchIndex;

use crate::{
    hashfs::HashFS, search_manager::SearchService, storage::backend::CascadingMediaBackend,
    LucilleAppError,
};

mod lucille_config;

pub use lucille_config::{ConfigBuilder, ConfigError, LucilleConfig};

#[derive(Debug)]
pub struct LucilleBuilder {
    pub config: lucille_config::ConfigBuilder,
}

impl LucilleBuilder {
    pub fn new() -> Result<Self, ConfigError> {
        Ok(LucilleBuilder {
            config: lucille_config::ConfigBuilder::new()?.load_environment(true),
        })
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn config_file(self, config_file: Option<&Path>) -> Result<Self, ConfigError> {
        self.update(|c| c.config_file(config_file))
    }

    fn update(
        self,
        f: impl FnOnce(
            lucille_config::ConfigBuilder,
        ) -> Result<lucille_config::ConfigBuilder, ConfigError>,
    ) -> Result<Self, ConfigError> {
        let LucilleBuilder {
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

    pub async fn build(self) -> Result<LucilleApp, LucilleAppError> {
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

        let hashfs = HashFS::new(config.media_root())?;
        let mut app = LucilleApp::new_with_hashfs(db, config, hashfs);

        #[cfg(feature = "aws-sdk")]
        app.add_s3_backend().await;

        log::trace!("{:#?}", app);
        Ok(app)
    }
}

#[derive(Debug)]
pub struct LucilleApp {
    pub db: Database,
    pub config: lucille_config::LucilleConfig,
    pub(crate) storage: CascadingMediaBackend,
}

impl LucilleApp {
    pub fn new(db: Database, config: lucille_config::LucilleConfig) -> Self {
        let storage = CascadingMediaBackend::default();
        LucilleApp {
            db,
            config,
            storage,
        }
    }
    pub fn new_with_hashfs(
        db: Database,
        config: lucille_config::LucilleConfig,
        hashfs: HashFS,
    ) -> Self {
        let mut storage = CascadingMediaBackend::default();
        storage.push_back(crate::storage::backend::DbStorageBackend::new(db.clone()));
        storage.push_back(crate::storage::backend::MediaRootBackend::new(hashfs));

        LucilleApp {
            db,
            config,
            storage,
        }
    }
    pub async fn add_hashfs(&mut self, hashfs: HashFS) {
        self.storage
            .push_back(crate::storage::backend::MediaRootBackend::new(hashfs))
    }
    #[cfg(feature = "aws-sdk")]
    pub async fn add_s3_backend(&mut self) {
        if let Some(bucket) = self.config.media_s3_bucket() {
            let cfg = aws_config::from_env().load().await;
            let backend = crate::storage::backend::S3MediaBackend::new(&cfg, bucket);
            self.storage.push_back(backend);
        }
    }

    pub fn search_service(&self, index_uuid: Uuid) -> Result<SearchService, LucilleAppError> {
        let index_dir = self.config.index_root().join(index_uuid.to_string());
        log::debug!("loading search index from: {:?}", index_dir.as_path());
        let index = SearchIndex::open_in_dir(index_uuid, index_dir)?;
        Ok(SearchService { index })
    }
}

#[cfg(test)]
pub mod tests {
    use lucille_config::ConfigBuilder;

    use super::*;

    pub struct LucilleTestApp {
        pub app: LucilleApp,
        pub dir: tempfile::TempDir,
    }

    pub async fn lucille_test_app() -> LucilleTestApp {
        let dir = tempfile::TempDir::new().expect("unable to create tmpdir");
        let db = Database::memory()
            .await
            .expect("could not build in memory database");

        let test_config =
            ConfigBuilder::new_test_config(dir.path()).expect("could not create test config");

        let app = LucilleApp {
            db,
            config: test_config,
            storage: CascadingMediaBackend::default(),
        };
        LucilleTestApp { app, dir }
    }
}

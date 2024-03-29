use std::path::Path;

use anyhow::Context;
use database::Database;
use lucille_core::uuid::Uuid;
use search::SearchIndex;

use crate::{
    hashfs::HashFS, search_manager::SearchService, storage::backend::CascadingMediaBackend,
};

mod lucille_config;

pub use lucille_config::{ConfigBuilder, LucilleConfig};

#[derive(Debug)]
pub struct LucilleBuilder {
    pub config: lucille_config::ConfigBuilder,
}

impl LucilleBuilder {
    pub fn new_with_user_dirs() -> anyhow::Result<Self> {
        Ok(LucilleBuilder {
            config: lucille_config::ConfigBuilder::new_with_user_dirs()?.load_environment(true),
        })
    }

    pub fn new_with_root(root: &Path) -> anyhow::Result<Self> {
        Ok(LucilleBuilder {
            config: lucille_config::ConfigBuilder::new_with_root(root)?.load_environment(true),
        })
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn config_file(self, config_file: Option<&Path>) -> anyhow::Result<Self> {
        self.update(|c| c.config_file(config_file))
    }

    fn update(
        self,
        f: impl FnOnce(lucille_config::ConfigBuilder) -> anyhow::Result<lucille_config::ConfigBuilder>,
    ) -> anyhow::Result<Self> {
        let LucilleBuilder {
            config: config_builder,
        } = self;
        let config_builder = f(config_builder)?;
        Ok(Self {
            config: config_builder,
        })
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn index_root(self, index_root: Option<&Path>) -> anyhow::Result<Self> {
        self.update(|c| c.index_root(index_root))
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn database_path(self, database_path: Option<&Path>) -> anyhow::Result<Self> {
        self.update(|c| c.database_path(database_path))
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn media_root(self, media_root: Option<&Path>) -> anyhow::Result<Self> {
        self.update(|c| c.media_root(media_root))
    }

    #[deprecated(note = "use the same function on the internal `config` object")]
    pub fn ffmpeg_override(self, ffmpeg: Option<&Path>) -> anyhow::Result<Self> {
        self.update(|c| c.ffmpeg_override(ffmpeg))
    }

    pub async fn build(self) -> anyhow::Result<LucilleApp> {
        let Self {
            config: config_builder,
        } = self;
        let config = config_builder.build().context("create config_builder")?;

        let db_opts = config
            .database_connection_opts()
            .context("create database opts")?;
        let mut db_builder = database::DatabaseBuider::default();
        db_builder.add_opts(db_opts).context("add db opts")?;
        db_builder.connect().await.context("connect to db")?;
        db_builder.migrate().await.context("migrate db schema")?;
        let (db, _) = db_builder.into_parts().context("database state error")?;

        let hashfs = HashFS::new(config.media_root()).context("create hashfs from media_root")?;
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

    pub fn search_service(&self, index_uuid: Uuid) -> anyhow::Result<SearchService> {
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

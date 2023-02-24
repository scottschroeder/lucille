use std::path::{Path, PathBuf};

use camino::Utf8Path;
use database::{Database, DatabaseFetcher};
use lucile_core::uuid::Uuid;
use search::SearchIndex;

use crate::{search_manager::SearchService, LucileAppError};

const QUALIFIER: &str = "io";
const ORGANIZATION: &str = "vauntware";
const APP: &str = "lucile";
const APP_CAPS: &str = "LUCILE";

const DATABASE_KEY: &str = "db";
const DEFAULT_DB_NAME: &str = "lucile.db";

const INDEX_ROOT_KEY: &str = "index_root";
const INDEX_DIR: &str = "index";

const MEDIA_ROOT_KEY: &str = "media_root";
const MEDIA_DIR: &str = "media";

const FFMPEG_CMD_KEY: &str = "ffmpeg";

const DEFAULT_CONFIG_FILE: &str = "lucile.toml";

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    ConfigError(#[from] config::ConfigError),
    #[error("unable to get user home directory")]
    NoUserHome,
    #[error("path is not utf8: {:?}", _0)]
    NonUtf8Path(PathBuf),
}

#[derive(Debug)]
pub struct LucileBuilder {
    dirs: directories::ProjectDirs,
    config_file: Option<PathBuf>,
    config_builder: config::ConfigBuilder<config::builder::DefaultState>,
}

fn camino_path(std_path: &Path) -> Result<&Utf8Path, ConfigError> {
    Utf8Path::from_path(std_path).ok_or_else(|| ConfigError::NonUtf8Path(std_path.to_path_buf()))
}
impl LucileBuilder {
    #[cfg(test)]
    fn new_test_config(root: &Path) -> Result<config::Config, ConfigError> {
        let root = camino_path(root)?;
        let data_dir = root.join("app_data_dir");
        let config = config::Config::builder()
            .set_default(FFMPEG_CMD_KEY, Some("ffmpeg_not_available_in_unit_tests"))?
            .set_default(INDEX_ROOT_KEY, data_dir.join(INDEX_DIR).as_str())?
            .set_default(MEDIA_ROOT_KEY, data_dir.join(MEDIA_DIR).as_str())?
            .build()?;
        Ok(config)
    }
    pub fn new() -> Result<Self, ConfigError> {
        let dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APP)
            .ok_or(ConfigError::NoUserHome)?;
        let data_dir = camino_path(dirs.data_dir())?;
        let mut config_builder = config::Config::builder()
            .set_default(FFMPEG_CMD_KEY, Option::<&str>::None)?
            .set_default(INDEX_ROOT_KEY, data_dir.join(INDEX_DIR).as_str())?
            .set_default(DATABASE_KEY, data_dir.join(DEFAULT_DB_NAME).as_str())?
            .set_default(MEDIA_ROOT_KEY, data_dir.join(MEDIA_DIR).as_str())?;

        if let Ok(db) = std::env::var(database::DATABASE_ENV_VAR) {
            config_builder = config_builder.set_default(DATABASE_KEY, db)?
        };

        Ok(Self {
            dirs,
            config_file: None,
            config_builder,
        })
    }

    fn set_path_override(mut self, key: &str, path: Option<&Path>) -> Result<Self, ConfigError> {
        let path_override = path.map(camino_path).transpose()?;
        let str_override = path_override.as_ref().map(|c| c.as_str());
        self.config_builder = self.config_builder.set_override_option(key, str_override)?;
        Ok(self)
    }

    pub fn config_file(mut self, config_file: Option<&Path>) -> Self {
        self.config_file = config_file.map(|p| p.to_path_buf());
        self
    }

    pub fn index_root(self, index_root: Option<&Path>) -> Result<Self, ConfigError> {
        self.set_path_override(INDEX_ROOT_KEY, index_root)
    }

    pub fn database_path(self, database_path: Option<&Path>) -> Result<Self, ConfigError> {
        self.set_path_override(DATABASE_KEY, database_path)
    }

    pub fn media_root(self, media_root: Option<&Path>) -> Result<Self, ConfigError> {
        self.set_path_override(MEDIA_ROOT_KEY, media_root)
    }

    pub fn ffmpeg_override(self, ffmpeg: Option<&Path>) -> Result<Self, ConfigError> {
        self.set_path_override(FFMPEG_CMD_KEY, ffmpeg)
    }

    pub async fn build(mut self) -> Result<LucileApp, LucileAppError> {
        let cfg_file = self
            .config_file
            .unwrap_or_else(|| self.dirs.config_dir().join(DEFAULT_CONFIG_FILE));
        if cfg_file.exists() {
            self.config_builder = self.config_builder.add_source(config::File::from(cfg_file));
        }
        let cfg = self
            .config_builder
            .add_source(config::Environment::with_prefix(APP_CAPS))
            .build()
            .map_err(ConfigError::from)?;
        log::info!("{:#?}", cfg);
        let db_path = cfg
            .get_string(DATABASE_KEY)
            .map_err(ConfigError::ConfigError)?;
        let db = DatabaseFetcher::from_path(db_path).await?;
        let app = LucileApp {
            db: db.db,
            config: cfg,
        };
        Ok(app)
    }
}

#[derive(Debug)]
pub struct LucileApp {
    pub db: Database,
    config: config::Config,
}

impl LucileApp {
    pub fn search_service(&self, index_uuid: Uuid) -> Result<SearchService, LucileAppError> {
        let index_dir = self.index_root().join(index_uuid.to_string());
        log::trace!("loading search index from: {:?}", index_dir.as_path());
        let index = SearchIndex::open_in_dir(index_uuid, index_dir)?;
        Ok(SearchService { index })
    }
}

impl LucileApp {
    fn get_path(&self, key: &str) -> PathBuf {
        PathBuf::from(self.config.get_string(key).unwrap())
    }
    pub fn index_root(&self) -> PathBuf {
        self.get_path(INDEX_ROOT_KEY)
    }
    pub fn media_root(&self) -> PathBuf {
        self.get_path(MEDIA_ROOT_KEY)
    }
    pub fn ffmpeg(&self) -> crate::ffmpeg::FFMpegBinary {
        match self.config.get::<Option<String>>(FFMPEG_CMD_KEY) {
            Ok(Some(s)) => crate::ffmpeg::FFMpegBinary::new(s),
            Ok(None) => crate::ffmpeg::FFMpegBinary::default(),
            Err(e) => panic!("{}", e),
        }
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
        let db_fetch = DatabaseFetcher::memory()
            .await
            .expect("could not build in memory database");

        let test_config =
            LucileBuilder::new_test_config(dir.path()).expect("could not create test config");

        let app = LucileApp {
            db: db_fetch.db,
            config: test_config,
        };
        LucileTestApp { app, dir }
    }
}

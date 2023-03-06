use std::path::{Path, PathBuf};

use camino::{Utf8Path, Utf8PathBuf};
use database::DatabaseError;

const QUALIFIER: &str = "io";
const ORGANIZATION: &str = "vauntware";
const APP: &str = "lucille";
const APP_CAPS: &str = "LUCILLE";

const DATABASE_KEY: &str = "db";
const DEFAULT_DB_NAME: &str = "lucille.db";

const INDEX_ROOT_KEY: &str = "index_root";
const INDEX_DIR: &str = "index";

const MEDIA_ROOT_KEY: &str = "media_root";
const MEDIA_DIR: &str = "media";

const MEDIA_S3_BUCKET_KEY: &str = "media_s3_bucket";
const GIF_UPLOAD_S3_BUCKET_KEY: &str = "gif_upload_s3_bucket";

const FFMPEG_CMD_KEY: &str = "ffmpeg";
const MEDIA_VIEW_KEY: &str = "media_view_priority";

const DEFAULT_CONFIG_FILE: &str = "lucille.toml";

type ExtConfigBuilder = config::ConfigBuilder<config::builder::DefaultState>;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error(transparent)]
    ConfigError(#[from] config::ConfigError),
    #[error("unable to get user home directory")]
    NoUserHome,
    #[error("path is not utf8: {:?}", _0)]
    NonUtf8Path(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    config_dir: Utf8PathBuf,
    config_path: Option<Utf8PathBuf>,
    load_environment: bool,
    config_builder: ExtConfigBuilder,
}

fn camino_path(std_path: &Path) -> Result<&Utf8Path, ConfigError> {
    Utf8Path::from_path(std_path).ok_or_else(|| ConfigError::NonUtf8Path(std_path.to_path_buf()))
}

fn new_config_builder(data_dir: &Utf8Path) -> ExtConfigBuilder {
    // unwraps are if our KEYs are not strings. These are statics, so its safe.
    config::Config::builder()
        .set_default(FFMPEG_CMD_KEY, Option::<&str>::None)
        .unwrap()
        .set_default(INDEX_ROOT_KEY, data_dir.join(INDEX_DIR).as_str())
        .unwrap()
        .set_default(DATABASE_KEY, data_dir.join(DEFAULT_DB_NAME).as_str())
        .unwrap()
        .set_default(MEDIA_ROOT_KEY, data_dir.join(MEDIA_DIR).as_str())
        .unwrap()
}

impl ConfigBuilder {
    #[cfg(test)]
    pub fn new_test_config(root: &Path) -> Result<LucilleConfig, ConfigError> {
        let root = camino_path(root)?;
        let data_dir = root.join("app_data_dir");
        let config = new_config_builder(&data_dir)
            .set_override(FFMPEG_CMD_KEY, "no_ffmpeg_in_tests")
            .unwrap()
            .build()?;
        Ok(LucilleConfig { inner: config })
    }

    pub fn new() -> Result<Self, ConfigError> {
        let dirs = directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APP)
            .ok_or(ConfigError::NoUserHome)?;
        let data_dir = camino_path(dirs.data_dir())?;
        let config_dir = camino_path(dirs.config_dir())?.to_path_buf();
        let config_builder = new_config_builder(data_dir);
        let builder = Self {
            load_environment: false,
            config_path: None,
            config_dir,
            config_builder,
        };

        Ok(builder)
    }

    /// Should we load configuration from the environment?
    pub fn load_environment(mut self, load_environment: bool) -> Self {
        self.load_environment = load_environment;
        self
    }

    fn set_path_override(mut self, key: &str, path: Option<&Path>) -> Result<Self, ConfigError> {
        let path_override = path.map(camino_path).transpose()?;
        let str_override = path_override.as_ref().map(|c| c.as_str());
        self.config_builder = self
            .config_builder
            .set_override_option(key, str_override)
            .unwrap();
        Ok(self)
    }

    pub fn config_file(mut self, config_file: Option<&Path>) -> Result<Self, ConfigError> {
        self.config_path = config_file
            .map(|p| camino_path(p).map(|p| p.to_path_buf()))
            .transpose()?;
        Ok(self)
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

    pub fn build(mut self) -> Result<LucilleConfig, ConfigError> {
        let cfg_file = self
            .config_path
            .unwrap_or_else(|| self.config_dir.join(DEFAULT_CONFIG_FILE));

        if cfg_file.exists() {
            self.config_builder = self
                .config_builder
                .add_source(config::File::from(cfg_file.as_std_path()));
        }

        if self.load_environment {
            // if let Ok(db) = std::env::var(database::DATABASE_ENV_VAR) {
            //     self.config_builder = self.config_builder.set_default(DATABASE_KEY, db)?
            // };

            self.config_builder = self
                .config_builder
                .add_source(config::Environment::with_prefix(APP_CAPS))
        }

        let lucille_cfg = LucilleConfig {
            inner: self.config_builder.build().map_err(ConfigError::from)?,
        };
        log::trace!("{:#?}", lucille_cfg);
        Ok(lucille_cfg)
    }
}

#[derive(Debug, Clone)]
pub struct LucilleConfig {
    inner: config::Config,
}

impl LucilleConfig {
    fn get_path(&self, key: &str) -> PathBuf {
        PathBuf::from(self.inner.get_string(key).unwrap())
    }
    fn export(&self) {
        todo!()
    }

    pub fn database_path(&self) -> String {
        self.inner.get_string(DATABASE_KEY).unwrap()
    }
    pub fn database_connection_opts(
        &self,
    ) -> Result<database::LucilleDbConnectOptions, DatabaseError> {
        let url = self.database_path();
        if url.starts_with("sqlite:") {
            database::LucilleDbConnectOptions::from_url(&url)
        } else {
            Ok(database::LucilleDbConnectOptions::from_path(&url))
        }
    }
    pub fn index_root(&self) -> PathBuf {
        self.get_path(INDEX_ROOT_KEY)
    }
    pub fn media_root(&self) -> PathBuf {
        self.get_path(MEDIA_ROOT_KEY)
    }
    pub fn media_s3_bucket(&self) -> Option<String> {
        self.inner.get_string(MEDIA_S3_BUCKET_KEY).ok()
    }
    pub fn output_s3_bucket(&self) -> Option<String> {
        self.inner.get_string(GIF_UPLOAD_S3_BUCKET_KEY).ok()
    }
    pub fn ffmpeg(&self) -> crate::ffmpeg::FFMpegBinary {
        match self.inner.get::<Option<String>>(FFMPEG_CMD_KEY) {
            Ok(Some(s)) => crate::ffmpeg::FFMpegBinary::new(s),
            Ok(None) => crate::ffmpeg::FFMpegBinary::default(),
            Err(e) => panic!("{}", e),
        }
    }
    pub fn media_view_priority(&self) -> Vec<String> {
        if let Ok(v) = self.inner.get_string(MEDIA_VIEW_KEY) {
            return vec![v];
        }
        self.inner
            .get_array(MEDIA_VIEW_KEY)
            .unwrap_or_else(|_| Vec::new())
            .into_iter()
            .map(|v| v.into_string())
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_else(|_| Vec::new())
    }
}

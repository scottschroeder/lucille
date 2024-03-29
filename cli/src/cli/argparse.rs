use anyhow::Context;
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug, Clone)]
pub struct FileCheckSettings {
    /// how rigorously should we validate storage files
    #[clap(long, value_enum, default_value_t = ArgFileCheckStrategy::CheckExists)]
    pub check_strategy: ArgFileCheckStrategy,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ArgFileCheckStrategy {
    /// Verify all files by re-calculating the hash
    VerifyAll,
    /// If the filename matches the expected hash,
    /// skip re-calculating the full hash
    TrustNameIsHash,
    /// Only check that the file exists, do not verify hashes
    CheckExists,
}

impl ArgFileCheckStrategy {
    pub(crate) fn to_app(&self) -> app::storage::verify::FileCheckStrategy {
        match self {
            ArgFileCheckStrategy::VerifyAll => app::storage::verify::FileCheckStrategy::VerifyAll,
            ArgFileCheckStrategy::TrustNameIsHash => {
                app::storage::verify::FileCheckStrategy::TrustNameIsHash
            }
            ArgFileCheckStrategy::CheckExists => {
                app::storage::verify::FileCheckStrategy::CheckExists
            }
        }
    }
}

#[derive(Parser, Debug, Default)]
pub struct AppConfig {
    #[clap(flatten)]
    pub config_file: AppConfigFile,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,

    #[clap(flatten)]
    pub media_root: MediaStorage,

    #[clap(flatten)]
    pub ffmpeg: FFMpegConfig,
}

impl AppConfig {
    pub async fn build_app(&self) -> anyhow::Result<app::app::LucilleApp> {
        let config = app::app::ConfigBuilder::new_with_user_dirs()?
            .load_environment(true)
            .config_file(self.config_file.config_file())?
            .ffmpeg_override(self.ffmpeg.ffmpeg())?
            .database_path(self.db.database_path())?
            .index_root(self.storage.index_root())?
            .media_root(self.media_root.media_root())?
            .build()?;

        let db_opts = config
            .database_connection_opts()
            .context("failed to create db opts")?;
        let mut db_builder = database::DatabaseBuider::default();
        db_builder
            .add_opts(db_opts)
            .context("database configuration")?;
        db_builder.connect().await.context("connect to db")?;
        db_builder
            .migrate()
            .await
            .context("validate db migrations")?;
        let (db, _) = db_builder.into_parts()?;
        let hashfs = app::hashfs::HashFS::new(config.media_root())?;
        let mut app = app::app::LucilleApp::new_with_hashfs(db, config, hashfs);
        app.add_s3_backend().await;
        Ok(app)
    }
}

#[derive(Parser, Debug, Default)]
pub struct DatabaseConfig {
    /// Path to sqlite database file.
    ///
    /// If not provided, will attempt to read `DATABASE_URL` env var, then user dirs.
    #[clap(long)]
    pub database_path: Option<std::path::PathBuf>,
}

impl DatabaseConfig {
    pub fn database_path(&self) -> Option<&std::path::Path> {
        self.database_path.as_deref()
    }
}

#[derive(Parser, Debug, Default)]
pub struct StorageConfig {
    /// Path to search index directory
    ///
    /// If not provided, will use user dirs
    #[clap(long)]
    pub index_root: Option<std::path::PathBuf>,
}

impl StorageConfig {
    pub fn index_root(&self) -> Option<&std::path::Path> {
        self.index_root.as_deref()
    }
}

#[derive(Parser, Debug, Default)]
pub struct MediaStorage {
    /// Path to local media storage
    ///
    /// If not provided, will use user dirs
    #[clap(long)]
    pub media_root: Option<std::path::PathBuf>,
}

impl MediaStorage {
    pub fn media_root(&self) -> Option<&std::path::Path> {
        self.media_root.as_deref()
    }
}

#[derive(Parser, Debug, Default)]
pub struct FFMpegConfig {
    /// Override binary called for `ffmpeg`
    #[clap(long)]
    pub ffmpeg: Option<std::path::PathBuf>,
}

impl FFMpegConfig {
    pub fn ffmpeg(&self) -> Option<&std::path::Path> {
        self.ffmpeg.as_deref()
    }
}

#[derive(Parser, Debug, Default)]
pub struct AppConfigFile {
    /// Path to application config file
    ///
    /// If not provided, will use user dirs
    #[clap(long)]
    config_file: Option<std::path::PathBuf>,
}

impl AppConfigFile {
    pub fn config_file(&self) -> Option<&std::path::Path> {
        self.config_file.as_deref()
    }
}

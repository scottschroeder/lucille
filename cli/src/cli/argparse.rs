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
    pub(crate) fn to_app(&self) -> app::storage::FileCheckStrategy {
        match self {
            ArgFileCheckStrategy::VerifyAll => app::storage::FileCheckStrategy::VerifyAll,
            ArgFileCheckStrategy::TrustNameIsHash => {
                app::storage::FileCheckStrategy::TrustNameIsHash
            }
            ArgFileCheckStrategy::CheckExists => app::storage::FileCheckStrategy::CheckExists,
        }
    }
}

#[derive(Parser, Debug)]
pub struct DatabaseConfig {
    /// Path to sqlite database file.
    ///
    /// If not provided, will attempt to read `DATABASE_URL` env var, then user dirs.
    #[clap(long)]
    pub database_path: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub struct StorageConfig {
    /// Path to search index directory
    ///
    /// If not provided, will use user dirs
    #[clap(long)]
    pub index_root: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub struct MediaStorage {
    /// Path to local media storage
    ///
    /// If not provided, will use user dirs
    #[clap(long)]
    pub media_root: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub struct FFMpegConfig {
    /// Override binary called for `ffmpeg`
    #[clap(long)]
    pub ffmpeg: Option<std::path::PathBuf>,
}

#[derive(Parser, Debug)]
pub enum MediaCommand {
    /// Scan raw media and create a new corpus
    Scan,
    /// Index corpus to create searchable database
    Index,
    /// Process raw media files for transcoding
    Prepare,
}

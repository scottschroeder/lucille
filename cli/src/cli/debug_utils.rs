use std::{str::FromStr, time::Duration};

use anyhow::Context;
use app::prepare::MediaProcessor;
use clap::Parser;
use lucile_core::metadata::MediaHash;

use super::argparse::{AppConfig, DatabaseConfig, StorageConfig};
use crate::cli::helpers;

#[derive(Parser, Debug)]
pub enum DebugCommand {
    /// Lookup all instances where a hash appears in the database
    HashLookup(HashLookup),

    /// Show the launch configuration/directories for the given settings.
    ShowConfig(ShowConfig),

    /// Split a media file into segments
    SplitMediaFile(SplitMediaFile),

    /// Decrypt a media file manually
    DecryptMediaFile(DecryptMediaFile),
}

#[derive(Parser, Debug)]
pub struct HashLookup {
    /// Search the database for this hash
    pub hash: String,

    #[clap(flatten)]
    pub db: DatabaseConfig,
}

#[derive(Parser, Debug)]
pub struct ShowConfig {
    #[clap(flatten)]
    pub cfg: AppConfig,
}

#[derive(Parser, Debug)]
pub struct SplitMediaFile {
    /// The input media file
    pub input: std::path::PathBuf,

    /// The the output directory
    pub output: std::path::PathBuf,

    /// The split duration target (may not be exact)
    #[clap(long)]
    pub duration: f32,

    /// Use the media splitter processing construct
    #[clap(long)]
    pub processor: bool,

    /// Encrypt the segments
    #[clap(long)]
    pub encrypt: bool,
}

#[derive(Parser, Debug)]
pub struct DecryptMediaFile {
    #[clap(flatten)]
    pub db: DatabaseConfig,

    /// The input media file
    pub input: std::path::PathBuf,

    /// The the output target
    pub output: std::path::PathBuf,

    /// A key to use for decryption, blank will search DB
    #[clap(long)]
    pub key: Option<String>,
}

impl DebugCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        match &self {
            DebugCommand::HashLookup(opts) => hash_lookup(opts).await,
            DebugCommand::ShowConfig(opts) => show_config(opts).await,
            DebugCommand::SplitMediaFile(opts) => split_media_file(opts).await,
            DebugCommand::DecryptMediaFile(opts) => decrypt_media_file(opts).await,
        }
    }
}
pub(crate) async fn show_config(args: &ShowConfig) -> anyhow::Result<()> {
    let app = args.cfg.build_app().await?;
    println!("{:#?}", app);
    Ok(())
}

pub(crate) async fn hash_lookup(args: &HashLookup) -> anyhow::Result<()> {
    let hash = MediaHash::from_str(&args.hash).context("could not parse hash")?;
    let app = helpers::get_app(Some(&args.db), None).await?;
    log::trace!("using app: {:?}", app);

    app::print_details_for_hash(&app, hash).await?;
    Ok(())
}

pub(crate) async fn decrypt_media_file(args: &DecryptMediaFile) -> anyhow::Result<()> {
    let mut f = tokio::io::BufReader::new(tokio::fs::File::open(args.input.as_path()).await?);
    let key = args
        .key
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("must provide key"))?;
    let key_data = lucile_core::encryption_config::KeyData::from_str(key)?;
    let mut plain_reader = app::encryption::decryptor(&key_data, &mut f).await?;

    let mut of = tokio::fs::File::create(args.output.as_path()).await?;
    tokio::io::copy(&mut plain_reader, &mut of).await?;
    Ok(())
}
pub(crate) async fn split_media_file(args: &SplitMediaFile) -> anyhow::Result<()> {
    let ffmpeg = app::ffmpeg::FFMpegBinary::default();
    if args.processor {
        let split_buider = app::prepare::MediaSplittingStrategy::new(
            ffmpeg,
            Duration::from_secs_f32(args.duration),
            if args.encrypt {
                app::prepare::Encryption::EasyAes
            } else {
                app::prepare::Encryption::None
            },
            &args.output,
        )?;
        let split_task = split_buider.split_task(args.input.as_path());
        let outcome = split_task.process().await?;
        println!("{:#?}", outcome);
        return Ok(());
    }

    if args.encrypt {
        anyhow::bail!("can not encrypt without processor");
    }

    let splitter = app::ffmpeg::split::FFMpegMediaSplit::new_with_output(
        &ffmpeg,
        &args.input,
        Duration::from_secs_f32(args.duration),
        &args.output,
    )?;
    log::info!("{:#?}", splitter);
    let outcome = splitter.run().await?;
    println!("{:#?}", outcome);
    Ok(())
}

use std::path::Path;

use anyhow::Context;
use app::hashfs::HashFS;
use clap::Parser;
use database::Database;
use lucile_core::metadata::MediaHash;

use super::argparse::MediaStorage;
use crate::cli::argparse::DatabaseConfig;

#[derive(Parser, Debug)]
pub enum CleanCommand {
    /// Remove unknown files from media root
    MediaRoot(CleanMediaRootCmd),
}

impl CleanCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        match self {
            CleanCommand::MediaRoot(cmd) => cmd.run().await,
        }
    }
}

#[derive(Parser, Debug)]
pub struct CleanMediaRootCmd {
    /// Do not perform deletion
    #[clap(long)]
    pub dry_run: bool,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub media_root: MediaStorage,
}

impl CleanMediaRootCmd {
    async fn run(&self) -> anyhow::Result<()> {
        let app = app::app::LucileBuilder::new()?
            .database_path(self.db.database_path())?
            .media_root(self.media_root.media_root())?
            .build()
            .await?;

        let hashfs = HashFS::new(app.media_root()).context("open HashFS")?;
        log::trace!("start get all hashes");
        let contents = hashfs
            .all_hashes()
            .await
            .context("fetch all hashes from HashFS")?;

        log::trace!("have all hashes, starting hash lookups");
        let mut set = tokio::task::JoinSet::new();

        for (p, hash) in contents {
            let db = app.db.clone();
            set.spawn(async move {
                check_existing_path_should_remove(&db, p.as_path(), hash)
                    .await
                    .map(|t| if t { Some((p, hash)) } else { None })
            });
        }

        let mut to_erase = Vec::new();
        while let Some(res) = set.join_next().await {
            let res = res.context("task join failure")??;
            if let Some(p) = res {
                to_erase.push(p)
            }
        }
        log::trace!("hash lookups done");

        for (p, _) in &to_erase {
            println!("erase: {:?}", p);
        }
        println!("total to erase = {}", to_erase.len());

        if self.dry_run {
            log::info!("not performing erase due to --dry-run");
            return Ok(());
        }

        let mut errs = 0;
        for (p, hash) in &to_erase {
            if let Err(e) = hashfs.remove(*hash).await {
                errs += 1;
                log::error!("error removing {:?} from hashfs: {}", p, e)
            }
        }
        if errs != 0 {
            anyhow::bail!("encountered errors removing files");
        }
        Ok(())
    }
}

async fn check_existing_path_should_remove(
    db: &Database,
    path: &Path,
    hash: MediaHash,
) -> anyhow::Result<bool> {
    let (segment, storage) = tokio::try_join!(
        db.get_media_segment_by_hash(hash),
        db.get_storage_by_hash(hash)
    )
    .context("looking up hash in db")?;

    if storage.as_ref().map(|s| s.path == path).unwrap_or(false) && segment.is_some() {
        Ok(false)
    } else if storage.is_none() && segment.is_none() {
        Ok(true)
    } else {
        log::warn!(
            "not sure what to do with {:?} segment: {:?} storage: {:?}",
            path,
            segment,
            storage
        );
        Ok(false)
    }
}

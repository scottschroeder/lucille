use app::DEFAULT_INDEX_WINDOW_SIZE;
use clap::Parser;

use super::argparse::{DatabaseConfig, StorageConfig};
use crate::cli::helpers;

#[derive(Parser, Debug)]
pub struct ScanChaptersOpts {
    /// Root directory to start recursive scan
    pub dir: std::path::PathBuf,

    /// If a filepath is already known to our database, trust the hash instead of re-computing
    #[clap(long)]
    pub trust_known_hashes: bool,

    /// Attach these files to an existing corpus
    #[clap(long)]
    pub corpus_name: String,

    #[clap(flatten)]
    pub db: DatabaseConfig,
}

#[derive(Parser, Debug)]
pub struct IndexCommand {
    pub corpus_name: String,

    #[clap(long, default_value_t=DEFAULT_INDEX_WINDOW_SIZE)]
    pub window_size: usize,

    #[clap(flatten)]
    pub db: DatabaseConfig,

    #[clap(flatten)]
    pub storage: StorageConfig,
}

impl ScanChaptersOpts {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&self.db), None).await?;
        let corpus = app.db.get_or_add_corpus(self.corpus_name.as_str()).await?;
        log::debug!("using corpus: {:?}", corpus);

        app.media_scanner(self.trust_known_hashes)
            .ingest(self.dir.as_path(), Some(&corpus))
            .await?;
        Ok(())
    }
}

impl IndexCommand {
    pub(crate) async fn run(&self) -> anyhow::Result<()> {
        let app = helpers::get_app(Some(&self.db), Some(&self.storage)).await?;
        log::trace!("using app: {:?}", app);
        let corpus_id = app
            .db
            .get_corpus_id(&self.corpus_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("could not find corpus: {:?}", self.corpus_name))?;

        let index = app::index_subtitles(&app, corpus_id, Some(self.window_size)).await?;
        println!("Created Index: {}", index.uuid);
        Ok(())
    }
}

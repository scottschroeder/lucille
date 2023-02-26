mod loader;
mod search_app;
use std::sync::Arc;

use anyhow::Context;
use app::app::LucileApp;
use search_app::SearchApp;

use self::loader::LoadedShell;

pub struct AppCtx<'a> {
    pub(crate) rt: &'a tokio::runtime::Handle,
    pub(crate) lucile: &'a std::sync::Arc<LucileApp>,
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct LucileShell {
    #[serde(skip)]
    app: Option<Arc<LucileApp>>,
    #[serde(skip)]
    pub(crate) rt: Option<tokio::runtime::Runtime>,
}

impl LucileShell {
    // fn get_ctx(&self) -> anyhow::Result<AppCtx<'_>> {
    //     let ctx_parts = self.rt.zip(self.app);
    // }
    fn start_runtime(&mut self) -> anyhow::Result<()> {
        if self.rt.is_some() {
            anyhow::bail!("runtime is already started");
        }
        log::info!("starting tokio runtime");
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("could not start tokio runtime")?;
        self.rt = Some(rt);
        Ok(())
    }

    fn load_app(&mut self) -> anyhow::Result<()> {
        if self.app.is_some() {
            anyhow::bail!("app is already loaded");
        }
        match &self.rt {
            Some(rt) => {
                let app = rt
                    .block_on(async { app::app::LucileBuilder::new()?.build().await })
                    .context("could not load lucile app")?;
                self.app = Some(Arc::new(app))
            }
            None => {
                anyhow::bail!("no tokio runtime available");
            }
        }
        Ok(())
    }
}

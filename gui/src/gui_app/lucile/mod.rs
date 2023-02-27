use std::sync::Arc;

use anyhow::Context;
use app::app::LucileApp;

use self::{
    loader::LoadManager,
    search_app::{load_last_index, SearchAppState},
};
use super::ErrorPopup;

mod gif_creation;
mod loader;
mod search_app;

pub struct LucileAppCtx<'a, T> {
    pub(crate) rt: &'a tokio::runtime::Handle,
    pub(crate) lucile: &'a std::sync::Arc<LucileApp>,
    outer: T,
}

pub trait LucileCtx {
    fn rt(&self) -> &tokio::runtime::Handle;
    fn app(&self) -> &Arc<LucileApp>;
}

impl<'a, T> LucileCtx for LucileAppCtx<'a, T> {
    fn rt(&self) -> &tokio::runtime::Handle {
        self.rt
    }

    fn app(&self) -> &Arc<LucileApp> {
        self.lucile
    }
}

impl<'a, T: ErrorPopup> ErrorPopup for LucileAppCtx<'a, T> {
    fn raise(&mut self, err: anyhow::Error) {
        self.outer.raise(err)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct LucileShell {
    // #[serde(skip)]
    // app: Option<Arc<LucileApp>>,
    // #[serde(skip)]
    // pub(crate) rt: LoadManager<tokio::runtime::Runtime>,
    #[serde(skip)]
    state: LucileRuntimeState,
    #[serde(skip)]
    search_app: SearchAppState,
}

enum LucileRuntimeState {
    Init {
        rt_loader: LoadManager<tokio::runtime::Runtime>,
    },
    Configure {
        rt: tokio::runtime::Runtime,
        app_loader: LoadManager<LucileApp>,
    },
    Ready {
        rt: tokio::runtime::Runtime,
        app: Arc<LucileApp>,
    },
}

impl LucileRuntimeState {
    fn get_rt_or_panic(self) -> tokio::runtime::Runtime {
        match self {
            LucileRuntimeState::Configure { rt, .. } => rt,
            LucileRuntimeState::Ready { rt, .. } => rt,
            _ => panic!("no runtime"),
        }
    }
    fn update<F: FnOnce(Self) -> Self>(&mut self, f: F) {
        let mut swp = Self::default();
        std::mem::swap(&mut swp, self);
        let mut out = f(swp);
        std::mem::swap(&mut out, self);
    }

    pub fn load_all(&mut self) -> anyhow::Result<()> {
        while self.load()? {}
        Ok(())
    }

    fn load(&mut self) -> anyhow::Result<bool> {
        match self {
            LucileRuntimeState::Init { rt_loader } => {
                let rt_opt = rt_loader
                    .aquire_owned(|| {
                        log::info!("starting tokio runtime");
                        tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .context("could not start tokio runtime")
                    })
                    .take()?;

                if let Some(rt) = rt_opt {
                    self.update(|_| LucileRuntimeState::Configure {
                        rt,
                        app_loader: LoadManager::Init,
                    });
                    return Ok(true);
                }
            }
            LucileRuntimeState::Configure { rt, app_loader } => {
                let opt_app = app_loader
                    .aquire_owned(|| {
                        rt.block_on(async { app::app::LucileBuilder::new()?.build().await })
                            .context("could not load lucile app")
                    })
                    .take()?;
                if let Some(app) = opt_app {
                    self.update(|state| LucileRuntimeState::Ready {
                        rt: state.get_rt_or_panic(),
                        app: Arc::new(app),
                    });
                    return Ok(true);
                }
            }
            LucileRuntimeState::Ready { .. } => {}
        }
        Ok(false)
    }
}

impl Default for LucileRuntimeState {
    fn default() -> Self {
        LucileRuntimeState::Init {
            rt_loader: LoadManager::Init,
        }
    }
}

impl LucileShell {
    pub fn load(&mut self) -> anyhow::Result<()> {
        self.state.load_all()
    }

    pub fn update_central_panel<Ctx: ErrorPopup>(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        appctx: &mut Ctx,
    ) {
        match &mut self.state {
            LucileRuntimeState::Init { rt_loader } => {
                if ui.button("Try Again?").clicked() {
                    rt_loader.reset();
                }
            }
            LucileRuntimeState::Configure { rt: _, app_loader } => {
                if ui.button("Try Again?").clicked() {
                    app_loader.reset();
                }
            }
            LucileRuntimeState::Ready { rt, app } => {
                let mut lucile_ctx = LucileAppCtx {
                    rt: rt.handle(),
                    lucile: app,
                    outer: appctx,
                };

                if let SearchAppState::Unknown = self.search_app {
                    self.search_app = load_last_index(&mut lucile_ctx);
                };

                if let SearchAppState::App(search_app) = &mut self.search_app {
                    search_app.update_central_panel(ui, &mut lucile_ctx)
                }
            }
        }
    }
}

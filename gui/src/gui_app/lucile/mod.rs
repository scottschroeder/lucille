use std::sync::Arc;

use anyhow::Context;
use app::app::LucileApp;

use self::{
    import_app::ImportApp,
    loader::LoadManager,
    runtime_loader::LucileConfigLoader,
    search_app::{load_last_index, SearchAppState},
};
use super::ErrorPopup;

mod gif_creation;
mod import_app;
mod loader;
mod runtime_loader;
mod search_app;

pub struct LucileAppCtx<'a, T> {
    pub(crate) rt: &'a tokio::runtime::Handle,
    pub(crate) lucile: &'a std::sync::Arc<LucileApp>,
    outer: T,
}

pub trait LucileCtx {
    /// Access to the tokio runtime
    fn rt(&self) -> &tokio::runtime::Handle;
    /// Access to the lucile configuration object
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
    import_app: ImportApp,
}

enum LucileRuntimeState {
    Init {
        rt_loader: LoadManager<tokio::runtime::Runtime>,
    },
    Configure {
        rt: tokio::runtime::Runtime,
        app_loader: runtime_loader::LucileConfigLoader,
    },
    Ready {
        rt: tokio::runtime::Runtime,
        app: Arc<LucileApp>,
    },
}

impl LucileRuntimeState {
    fn is_ready(&self) -> bool {
        matches!(self, LucileRuntimeState::Ready { .. })
    }
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
                        app_loader: LucileConfigLoader::default(),
                    });
                    return Ok(true);
                }
            }
            LucileRuntimeState::Configure { rt, app_loader } => {
                let opt_app = app_loader.run_autoload(rt.handle())?;
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
    pub fn update(&mut self, ctx: &mut impl ErrorPopup) -> anyhow::Result<()> {
        if let LucileRuntimeState::Ready { rt, app } = &mut self.state {
            let mut lucile_ctx = LucileAppCtx {
                rt: rt.handle(),
                lucile: app,
                outer: ctx,
            };
            self.import_app.update(&mut lucile_ctx)
        } else {
            self.state
                .load_all()
                .context("unable to launch application")?;
        }
        Ok(())
    }

    pub fn file_menu(&mut self, ui: &mut egui::Ui) {
        if !self.state.is_ready() {
            return;
        }

        if ui.button("Import").clicked() {
            self.import_app.open_app();
            ui.close_menu()
        }
    }

    pub fn update_central_panel<Ctx: ErrorPopup>(&mut self, ctx: &mut Ctx, ui: &mut egui::Ui) {
        self.import_app.ui(ui, ctx);

        match &mut self.state {
            LucileRuntimeState::Init { rt_loader } => {
                if ui.button("Try Again?").clicked() {
                    rt_loader.reset();
                }
            }
            LucileRuntimeState::Configure { rt, app_loader } => app_loader.ui(ui, rt.handle(), ctx),
            LucileRuntimeState::Ready { rt, app } => {
                let mut lucile_ctx = LucileAppCtx {
                    rt: rt.handle(),
                    lucile: app,
                    outer: ctx,
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

use std::sync::Arc;

use anyhow::Context;
use app::app::LucilleApp;

use self::{
    import_app::ImportApp,
    loader::LoadManager,
    runtime_loader::LucilleConfigLoader,
    search_app::{load_last_index, SearchAppState},
};
use super::ErrorPopup;

mod gif_creation;
mod import_app;
mod loader;
mod runtime_loader;
mod search_app;

pub struct LucilleAppCtx<'a, T> {
    pub(crate) rt: &'a tokio::runtime::Handle,
    pub(crate) lucille: &'a std::sync::Arc<LucilleApp>,
    outer: T,
}

pub trait LucilleCtx {
    /// Access to the tokio runtime
    fn rt(&self) -> &tokio::runtime::Handle;
    /// Access to the lucille configuration object
    fn app(&self) -> &Arc<LucilleApp>;
}

impl<'a, T> LucilleCtx for LucilleAppCtx<'a, T> {
    fn rt(&self) -> &tokio::runtime::Handle {
        self.rt
    }

    fn app(&self) -> &Arc<LucilleApp> {
        self.lucille
    }
}

impl<'a, T: ErrorPopup> ErrorPopup for LucilleAppCtx<'a, T> {
    fn raise(&mut self, err: anyhow::Error) {
        self.outer.raise(err)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct LucilleShell {
    // #[serde(skip)]
    // app: Option<Arc<LucilleApp>>,
    // #[serde(skip)]
    // pub(crate) rt: LoadManager<tokio::runtime::Runtime>,
    #[serde(skip)]
    state: LucilleRuntimeState,
    #[serde(skip)]
    search_app: SearchAppState,
    import_app: ImportApp,
}

enum LucilleRuntimeState {
    Init {
        rt_loader: LoadManager<tokio::runtime::Runtime>,
    },
    Configure {
        rt: tokio::runtime::Runtime,
        app_loader: runtime_loader::LucilleConfigLoader,
    },
    Ready {
        rt: tokio::runtime::Runtime,
        app: Arc<LucilleApp>,
    },
}

impl LucilleRuntimeState {
    fn is_ready(&self) -> bool {
        matches!(self, LucilleRuntimeState::Ready { .. })
    }
}

impl LucilleRuntimeState {
    fn get_rt_or_panic(self) -> tokio::runtime::Runtime {
        match self {
            LucilleRuntimeState::Configure { rt, .. } => rt,
            LucilleRuntimeState::Ready { rt, .. } => rt,
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
            LucilleRuntimeState::Init { rt_loader } => {
                let rt_opt = rt_loader
                    .aquire_owned(|| {
                        log::trace!("starting tokio runtime");
                        tokio::runtime::Builder::new_multi_thread()
                            .enable_all()
                            .build()
                            .context("could not start tokio runtime")
                    })
                    .take()?;

                if let Some(rt) = rt_opt {
                    self.update(|_| LucilleRuntimeState::Configure {
                        rt,
                        app_loader: LucilleConfigLoader::default(),
                    });
                    return Ok(true);
                }
            }
            LucilleRuntimeState::Configure { rt, app_loader } => {
                let opt_app = app_loader.run_autoload(rt.handle())?;
                if let Some(app) = opt_app {
                    self.update(|state| LucilleRuntimeState::Ready {
                        rt: state.get_rt_or_panic(),
                        app: Arc::new(app),
                    });
                    return Ok(true);
                }
            }
            LucilleRuntimeState::Ready { .. } => {}
        }
        Ok(false)
    }
}

impl Default for LucilleRuntimeState {
    fn default() -> Self {
        LucilleRuntimeState::Init {
            rt_loader: LoadManager::Init,
        }
    }
}

impl LucilleShell {
    pub fn update(&mut self, ctx: &mut impl ErrorPopup) -> anyhow::Result<()> {
        if let LucilleRuntimeState::Ready { rt, app } = &mut self.state {
            let mut lucille_ctx = LucilleAppCtx {
                rt: rt.handle(),
                lucille: app,
                outer: ctx,
            };
            let refresh = self.import_app.update(&mut lucille_ctx);
            if refresh {
                self.search_app = SearchAppState::Unknown;
            }
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
            LucilleRuntimeState::Init { rt_loader } => {
                if ui.button("Try Again?").clicked() {
                    rt_loader.reset();
                }
            }
            LucilleRuntimeState::Configure { rt, app_loader } => {
                app_loader.ui(ui, rt.handle(), ctx)
            }
            LucilleRuntimeState::Ready { rt, app } => {
                let mut lucille_ctx = LucilleAppCtx {
                    rt: rt.handle(),
                    lucille: app,
                    outer: ctx,
                };

                match &mut self.search_app {
                    SearchAppState::Unknown => {
                        self.search_app = load_last_index(&mut lucille_ctx);
                        if matches!(self.search_app, SearchAppState::None) {
                            self.import_app.open_app()
                        }
                    }
                    SearchAppState::App(search_app) => {
                        search_app.update_central_panel(ui, &mut lucille_ctx)
                    }
                    SearchAppState::None => {
                        ui.heading("no media available");
                    }
                }
            }
        }
    }
}

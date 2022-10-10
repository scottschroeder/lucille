// use hotkey_manager::HotKeyManager;
// use image_sorter::NamedImage;

use std::{path::Path, str::FromStr};

use anyhow::Context;
use app::{app::LucileApp, search_manager::SearchService};
use lucile_core::{export::CorpusExport, uuid::Uuid};

pub(crate) use self::search_app::SearchApp;
use self::{
    error::ErrorChainLogLine,
    loader::{load_last_index, LoadedShell},
};

pub mod error;
mod loader;
mod search_app;
// mod import_manager;

// pub(crate) mod lucileimage;
// pub(crate) mod components {
//     pub(crate) mod card;
//     pub(crate) mod choice;
// }
// mod database;
// mod hotkey_manager;
// mod image_sorter;
// mod lucile_app;
// mod lucile_manager;
//

pub struct AppCtx<'a> {
    pub(crate) rt: &'a tokio::runtime::Handle,
    pub(crate) lucile: &'a std::sync::Arc<LucileApp>,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct ShellApp {
    #[serde(skip)]
    loaded: Option<LoadedShell>,
    #[serde(skip)]
    search_app_state: SearchAppState,
    // #[serde(skip)]
    // hotkeys: HotKeyManager,
    // #[serde(skip)]
    // dirs: directories::ProjectDirs,
    // #[serde(skip)]
    // db: Option<database::AppData>,
}

enum SearchAppState {
    Unknown,
    None,
    App(SearchApp),
}

impl Default for ShellApp {
    fn default() -> Self {
        Self {
            loaded: None,
            search_app_state: SearchAppState::Unknown,
            // hotkeys: HotKeyManager::default(),
            // dirs: directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APP).unwrap(),
            // db: None,
        }
    }
}

async fn import_and_index(lucile: &LucileApp, packet: CorpusExport) -> anyhow::Result<()> {
    let cid = app::import_corpus_packet(&lucile, packet)
        .await
        .context("could not import packet")?;
    app::index_subtitles(&lucile, cid, None)
        .await
        .context("could not index subtitles")?;
    Ok(())
}

impl ShellApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let app: Self = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        app
    }
}

fn import(ctx: &mut AppCtx<'_>, selection: &Path) -> anyhow::Result<()> {
    let f = std::fs::File::open(selection)
        .with_context(|| format!("unable to open file {:?} for import", selection))?;
    let packet = serde_json::from_reader(f).context("could not deserialize import packet")?;
    let lucile = ctx.lucile.clone();
    ctx.rt
        .block_on(async { import_and_index(&lucile, packet).await })
        .context("unable to run import/index in background thread")?;
    Ok(())
}

impl eframe::App for ShellApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let Self {
            // lucile_manager,
            // hotkeys,
            // dirs,
            // db,
            search_app_state,
            loaded,
        } = self;

        let LoadedShell { rt, lucile } = match loaded {
            Some(s) => s,
            None => {
                let s = LoadedShell::load()
                    .context("failed to initialize GUI")
                    .unwrap();
                *loaded = Some(s);
                loaded.as_mut().unwrap()
            }
        };

        let mut app_ctx = AppCtx {
            rt: rt.handle(),
            lucile,
        };

        if let SearchAppState::Unknown = search_app_state {
            *search_app_state = load_last_index(&mut app_ctx);
        };

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Import").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_file() {
                            if let Err(e) = import(&mut app_ctx, p.as_path()) {
                                log::error!("{:?}", ErrorChainLogLine::from(e));
                            } else {
                                *search_app_state = SearchAppState::Unknown;
                            }
                            // selection = Some(LoaderSelection::Path(p));
                        }
                        ui.close_menu()
                    }
                    if ui.button("Quit").clicked() {
                        frame.quit();
                    }
                });
            });
        });

        egui::SidePanel::right("side_panel").show(ctx, |ui| {
            // lucile_manager.update_side_panel(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // app_ctx.hotkeys.check_cleared(ui);
            // lucile_manager.update_central_panel(ui, &mut app_ctx);
            if let SearchAppState::App(search_app) = search_app_state {
                search_app.update_central_panel(ui, &mut app_ctx);
            }
        });
    }
}

pub mod error;

mod lucile;

pub mod egui_logger;
pub mod error_popup;
use anyhow::Context;
pub use error_popup::ErrorPopup;

struct ShellCtx<'a> {
    error_manager: &'a mut error_popup::ErrorManager,
}

impl<'a> ErrorPopup for ShellCtx<'a> {
    fn raise(&mut self, err: anyhow::Error) {
        self.error_manager.raise(err)
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
#[derive(Default)]
pub struct ShellApp {
    lucile: lucile::LucileShell,
    show_logger: bool,
    logger_ui: egui_logger::LoggerUi,
    #[serde(skip)]
    error_manager: error_popup::ErrorManager,
    // #[serde(skip)]
    // search_app_state: SearchAppState,
    // #[serde(skip)]
    // hotkeys: HotKeyManager,
    // #[serde(skip)]
    // dirs: directories::ProjectDirs,
    // #[serde(skip)]
    // db: Option<database::AppData>,
}

impl ShellApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        cc.egui_ctx.set_visuals(egui::Visuals::light());

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

impl eframe::App for ShellApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let ShellApp {
            lucile,
            show_logger: _,
            logger_ui: _,
            error_manager,
        } = self;
        let mut app_ctx = ShellCtx { error_manager };
        app_ctx.handle_err(lucile.load().context("unable to launch application"));
        // let Self {
        //     // lucile_manager,
        //     // hotkeys,
        //     // dirs,
        //     // db,
        //     search_app_state,
        //     loaded,
        // } = self;

        // let LoadedShell { rt, lucile } = match loaded {
        //     Some(s) => s,
        //     None => {
        //         let s = LoadedShell::load()
        //             .context("failed to initialize GUI")
        //             .unwrap();
        //         *loaded = Some(s);
        //         loaded.as_mut().unwrap()
        //     }
        // };

        // let mut app_ctx = AppCtx {
        //     rt: rt.handle(),
        //     lucile,
        // };

        // if let SearchAppState::Unknown = search_app_state {
        //     *search_app_state = load_last_index(&mut app_ctx);
        // };

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    lucile.file_menu(ui);
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    // if ui.button("Import").clicked() {
                    //     if let Some(p) = rfd::FileDialog::new().pick_file() {
                    //         if let Err(e) = import(&mut app_ctx, p.as_path()) {
                    //             log::error!("{:?}", ErrorChainLogLine::from(e));
                    //         } else {
                    //             *search_app_state = SearchAppState::Unknown;
                    //         }
                    //         // selection = Some(LoaderSelection::Path(p));
                    //     }
                    //     ui.close_menu()
                    // }
                    if ui.button("Debug Logs").clicked() {
                        self.show_logger = !self.show_logger;
                        ui.close_menu()
                    }
                });
                ui.menu_button("Log Message", |ui| {
                    if ui.button("Trace").clicked() {
                        log::trace!("log message button clicked!");
                    }
                    if ui.button("Debug").clicked() {
                        log::debug!("log message button clicked!");
                    }
                    if ui.button("Info").clicked() {
                        log::info!("log message button clicked!");
                    }
                    if ui.button("Warn").clicked() {
                        log::warn!("log message button clicked!");
                    }
                    if ui.button("Error").clicked() {
                        log::error!("log message button clicked!");
                    }

                    if ui.button("Raise Simple Error").clicked() {
                        app_ctx.raise(anyhow::anyhow!("this is an error"))
                    }

                    if ui.button("Raise Layered Error").clicked() {
                        let root =
                            std::io::Error::new(std::io::ErrorKind::Other, "I/O device blocked");
                        let e = anyhow::Error::from(root)
                            .context("unable to read media")
                            .context("transcoding failed");

                        app_ctx.raise(e)
                    }
                });
            });
        });

        // egui::SidePanel::right("side_panel").show(ctx, |_ui| {
        //     // lucile_manager.update_side_panel(ui);
        // });

        egui::CentralPanel::default().show(ctx, |ui| {
            // app_ctx.hotkeys.check_cleared(ui);
            lucile.update_central_panel(ctx, ui, &mut app_ctx);
            // if let SearchAppState::App(search_app) = search_app_state {
            //     search_app.update_central_panel(ui, &mut app_ctx);
            // }
        });

        egui::Window::new("Debug Logs")
            .open(&mut self.show_logger)
            .show(ctx, |ui| {
                // draws the logger ui.
                self.logger_ui.ui(ui);
                // egui_logger::logger_ui(ui);
            });
        self.error_manager.show(ctx);
    }
}

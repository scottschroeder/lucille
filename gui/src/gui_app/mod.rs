pub mod error;

mod lucile;

pub mod egui_logger;
pub mod error_popup;
pub mod oneshot_state;
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

        if let Err(e) = lucile.update(&mut app_ctx) {
            app_ctx.raise(e);
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    lucile.file_menu(ui);
                    if ui.button("Quit").clicked() {
                        frame.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Debug Logs").clicked() {
                        self.show_logger = !self.show_logger;
                        ui.close_menu()
                    }
                });
            });
        });

        // egui::SidePanel::right("side_panel").show(ctx, |_ui| {
        //     // lucile_manager.update_side_panel(ui);
        // });

        egui::CentralPanel::default().show(ctx, |ui| {
            lucile.update_central_panel(&mut app_ctx, ui);
        });

        egui::Window::new("Debug Logs")
            .open(&mut self.show_logger)
            .show(ctx, |ui| {
                self.logger_ui.ui(ui);
            });
        self.error_manager.show(ctx);
    }
}

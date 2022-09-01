// use hotkey_manager::HotKeyManager;
// use image_sorter::NamedImage;

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


pub struct AppCtx<'a> {
    pub(crate) inner: &'a str
    // pub(crate) db: &'a mut database::AppData,
    // pub(crate) hotkeys: &'a mut HotKeyManager,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct ShellApp {
    // lucile_manager: lucile_manager::LucileManager,
    // #[serde(skip)]
    // hotkeys: HotKeyManager,
    // #[serde(skip)]
    // dirs: directories::ProjectDirs,
    // #[serde(skip)]
    // db: Option<database::AppData>,
}

impl Default for ShellApp {
    fn default() -> Self {
        Self {
            // lucile_manager: lucile_manager::LucileManager::default(),
            // hotkeys: HotKeyManager::default(),
            // dirs: directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APP).unwrap(),
            // db: None,
        }
    }
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
        } = self;

        // let db = match db {
        //     Some(db) => db,
        //     None => {
        //         // TODO what happens if we cant create db?
        //         let new_db = database::AppData::open(dirs.data_dir()).unwrap();
        //         new_db.init().unwrap();
        //         *db = Some(new_db);
        //         db.as_mut().unwrap()
        //     }
        // };

        // let mut app_ctx = AppCtx { db, hotkeys };

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    // if lucile_manager.is_loaded() && ui.button("Unload Current ").clicked() {
                    //     lucile_manager.unload()
                    // }
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
        });
    }
}

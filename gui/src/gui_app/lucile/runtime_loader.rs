use anyhow::Context;
use app::app::LucileApp;
use database::DatabaseConnectState;
use egui::RichText;

use crate::gui_app::{error_popup::ErrorUi, ErrorPopup};

#[derive(Default)]
enum ConfigState {
    #[default]
    Init,
    Builder(app::app::ConfigBuilder),
    Configured(app::app::LucileConfig),
}

#[derive(Default)]
pub struct LucileConfigLoader {
    manual_loading: bool,
    config: ConfigState,
    db: database::DatabaseBuider,
    delete_db: bool,
}

impl LucileConfigLoader {
    fn advance_state(&mut self, rt: &tokio::runtime::Handle) -> anyhow::Result<Option<LucileApp>> {
        match &mut self.config {
            ConfigState::Init => {
                self.config = ConfigState::Builder(
                    app::app::ConfigBuilder::new().context("could not create new ConfigBuilder")?,
                );
            }
            ConfigState::Builder(b) => {
                self.config =
                    ConfigState::Configured(b.clone().build().context("could not create Config")?)
            }
            ConfigState::Configured(c) => match self.db.current_state() {
                DatabaseConnectState::Init => {
                    let db_path = c.database_path();
                    let opts = database::LucileDbConnectOptions::from_path(db_path);
                    self.db.add_opts(opts)?;
                }
                DatabaseConnectState::Configured => {
                    rt.block_on(async { self.db.connect().await })?;
                }
                DatabaseConnectState::Connected => {
                    rt.block_on(async { self.db.migrate().await })?;
                }
                DatabaseConnectState::Ready => {
                    let (db, _) = self.db.clone().into_parts()?;
                    return Ok(Some(LucileApp {
                        config: c.clone(),
                        db,
                    }));
                }
            },
        }
        Ok(None)
    }

    fn get_app(&self) -> Option<LucileApp> {
        if let ConfigState::Configured(ref config) = self.config {
            if self.db.current_state() == DatabaseConnectState::Ready {
                if let Ok((db, _)) = self.db.clone().into_parts() {
                    return Some(LucileApp {
                        config: config.clone(),
                        db,
                    });
                }
            }
        }
        None
    }

    pub fn run_autoload(
        &mut self,
        rt: &tokio::runtime::Handle,
    ) -> anyhow::Result<Option<LucileApp>> {
        while !self.manual_loading && self.db.current_state() != DatabaseConnectState::Ready {
            match self.advance_state(rt) {
                Ok(Some(x)) => return Ok(Some(x)),
                Ok(None) => {}
                Err(e) => {
                    self.manual_loading = true;
                    return Err(e).context("failure loading LucileApp");
                }
            }
        }
        Ok(self.get_app())
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        rt: &tokio::runtime::Handle,
        ctx: &mut impl ErrorPopup,
    ) {
        match &mut self.config {
            ConfigState::Init => match app::app::ConfigBuilder::new() {
                Ok(c) => log::error!("ui should not be stuck in config init: {:?}", c),
                Err(e) => {
                    SimpleMsgAndRetryUi {
                        header: "Error initializing the configuration loader",
                        body: "",
                        err: Some(anyhow::Error::from(e)),
                    }
                    .ui(ui, &mut self.manual_loading);
                }
            },
            ConfigState::Builder(b) => {
                SimpleMsgAndRetryUi {
                    header: "Error building the configuration",
                    body: &format!("{:#?}", b),
                    err: None,
                }
                .ui(ui, &mut self.manual_loading);
            }
            ConfigState::Configured(c) => {
                let db_path = c.database_path();
                let opts = database::LucileDbConnectOptions::from_path(&db_path);
                match self.db.current_state() {
                    DatabaseConnectState::Init => {
                        SimpleMsgAndRetryUi {
                            header: "Could not configure database opts",
                            body: &format!("{}:\n{:#?}", db_path, opts),
                            err: None,
                        }
                        .ui(ui, &mut self.manual_loading);
                    }
                    DatabaseConnectState::Configured => {
                        SimpleMsgAndRetryUi {
                            header: "Could not connect to database",
                            body: &format!("{}:\n{:#?}", db_path, opts),
                            err: None,
                        }
                        .ui(ui, &mut self.manual_loading);
                    }
                    DatabaseConnectState::Connected => {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Could not migrate database to latest version")
                                    .heading(),
                            );
                            if ui.button("Try Again?").clicked() {
                                self.manual_loading = false;
                            }

                            if ui.button("Delete Database").clicked() {
                                self.delete_db = true;
                            }
                        });
                        if self.delete_db {
                            self.delete_database_ui(ui, &db_path, rt, ctx)
                        }
                        if let Some(migrations) = self.db.get_migration_results() {
                            migration_ui(migrations, ui);
                        }
                    }
                    DatabaseConnectState::Ready => {
                        log::error!("ui should not be stuck in config ready");
                    }
                }
            }
        }
    }
    fn delete_database_ui(
        &mut self,
        ui: &mut egui::Ui,
        db_path: &str,
        _rt: &tokio::runtime::Handle,
        ctx: &mut impl ErrorPopup,
    ) {
        egui::Window::new("Confirm Delete Database").show(ui.ctx(), |ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("This will DELETE everything in the database!").strong());

                ui.label(
                    RichText::new(format!(
                        "You can backup {:?} if you want to save anything",
                        db_path
                    ))
                    .weak(),
                );

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.delete_db = false;
                    }
                    if ui.button("Delete").clicked() {
                        self.delete_db = false;
                        // let url = format!("sqlite:{}", db_path);
                        // let res = std::fs::File::create(db_path);
                        let res = std::fs::rename(db_path, format!("{}.bak", db_path));
                        // let res = rt.block_on(async {
                        //     database::drop_everything_PROBABLY_DONT_USE(&url).await
                        // });
                        match res {
                            Ok(_) => {
                                self.manual_loading = false;
                                self.db = database::DatabaseBuider::default();
                            }
                            Err(e) => ctx.raise(e.into()),
                        }
                    }
                })
            })
        });
    }
}

fn migration_ui(migrations: &[database::MigrationRecord], ui: &mut egui::Ui) {
    const GREEN_CHECK: &str = "✅";
    const RED_X: &str = "❌";
    let mark_truthy = |b: bool| {
        if b {
            GREEN_CHECK
        } else {
            RED_X
        }
    };
    egui::ScrollArea::vertical()
        .auto_shrink([true, true])
        .stick_to_bottom(true)
        .enable_scrolling(true)
        .show(ui, |ui| {
            egui::Grid::new("migration_ui").show(ui, |ui| {
                ui.label(RichText::new("Migration ID").strong());
                ui.label(RichText::new("Description").strong());
                ui.label(RichText::new("Required").strong());
                ui.label(RichText::new("Applied").strong());
                ui.end_row();
                for r in migrations {
                    ui.label(r.id.to_string());
                    ui.label(r.description.as_deref().unwrap_or("<UNKNOWN>"));
                    ui.label(mark_truthy(r.resolved));
                    ui.label(mark_truthy(r.applied));
                    ui.end_row();
                }
            });
        });
}

fn migrate_record_ui(_r: &database::MigrationRecord, _ui: &mut egui::Ui) {}

struct SimpleMsgAndRetryUi<'a> {
    header: &'a str,
    body: &'a str,
    err: Option<anyhow::Error>,
}

impl<'a> SimpleMsgAndRetryUi<'a> {
    fn ui(self, ui: &mut egui::Ui, retry: &mut bool) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.header).heading());
            if ui.button("Try Again?").clicked() {
                // seems backwards, but is correct
                *retry = false;
            }
        });
        let rich = RichText::new(self.body).text_style(egui::style::TextStyle::Monospace);
        ui.add(egui::Label::new(rich).wrap(true));
        if let Some(e) = self.err {
            let mut err_ui = ErrorUi::from(e);
            err_ui.ui(ui)
        }
    }
}

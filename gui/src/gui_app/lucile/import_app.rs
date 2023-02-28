use anyhow::Context;

use crate::gui_app::ErrorPopup;

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct ImportApp {
    open: bool,
    src: String,
}

impl ImportApp {
    pub fn open_app(&mut self) {
        self.open = true;
        log::debug!("open import");
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut impl ErrorPopup) {
        egui::Window::new("Import")
            .open(&mut self.open)
            .show(ui.ctx(), |ui| {
                ui.heading("Import Location");
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.src);
                    if ui.button("Choose File").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_file() {
                            match camino::Utf8PathBuf::from_path_buf(p)
                                .map_err(|e| anyhow::anyhow!("path is not utf8: {:?}", e))
                                .context("unable to use selected file path")
                            {
                                Ok(p) => self.src = p.into_string(),
                                Err(e) => ctx.raise(e),
                            }
                        }
                    }
                })
            });
    }
}

use anyhow::Context;
use app::app::LucileApp;
use lucile_core::{export::CorpusExport, identifiers::CorpusId};
use tokio::sync::oneshot::Receiver;

use crate::gui_app::{
    oneshot_state::{OneshotManager, OneshotState},
    ErrorPopup,
};

use super::LucileCtx;

type TxRecv<T> = Receiver<anyhow::Result<T>>;

#[derive(Debug, Clone)]
enum ImportObject {
    CorpusExport(CorpusExport),
}

async fn load_object(app: &LucileApp, src: &str) -> anyhow::Result<ImportObject> {
    let f = tokio::fs::File::open(src)
        .await
        .context("file could not be opened")?;
    let mut buf = tokio::io::BufReader::new(f);
    let mut data = Vec::new();
    tokio::io::copy(&mut buf, &mut data)
        .await
        .context("could not read file")?;

    let packet: CorpusExport =
        serde_json::from_slice(&data).context("could not deserialize import packet")?;

    Ok(ImportObject::CorpusExport(packet))
}

async fn import_object(
    app: &LucileApp,
    obj: &ImportObject,
    update_index: bool,
) -> anyhow::Result<()> {
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    anyhow::bail!("not doing import");
    Ok(())
}

#[derive(Default)]
enum LoadState<T> {
    #[default]
    Init,
    Triggered,
    Waiting(TxRecv<T>),
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct ImportApp {
    open: bool,
    #[serde(skip)]
    src: String,
    #[serde(skip)]
    object: Option<ImportObject>,
    #[serde(skip)]
    state_obj_load: LoadState<ImportObject>,
    skip_index: bool,
    #[serde(skip)]
    state_import: OneshotManager<ImportObject, ()>,
}

impl ImportApp {
    pub fn open_app(&mut self) {
        self.open = true;
        log::debug!("open import");
    }

    fn reset(&mut self) {
        let mut swp = ImportApp::default();
        std::mem::swap(&mut swp, self);
    }

    pub fn update(&mut self, ctx: &mut (impl LucileCtx + ErrorPopup)) {
        match &mut self.state_obj_load {
            LoadState::Init => {}
            LoadState::Triggered => {
                let rt = ctx.rt();
                let (tx, rx) = tokio::sync::oneshot::channel();
                let src = self.src.clone();
                let app = ctx.app().clone();
                rt.spawn(async move {
                    let res = load_object(&app, &src)
                        .await
                        .context("unable to load import");
                    _ = tx.send(res)
                });
                self.state_obj_load = LoadState::Waiting(rx);
            }
            LoadState::Waiting(rx) => {
                let mut close = true;
                match rx.try_recv() {
                    Ok(Ok(obj)) => {
                        self.object = Some(obj);
                    }
                    Ok(Err(e)) => ctx.raise(e),
                    Err(tokio::sync::oneshot::error::TryRecvError::Empty) => close = false,
                    Err(e) => ctx.raise(
                        anyhow::Error::from(e)
                            .context("did not recieve a message from import thread"),
                    ),
                };
                if close {
                    self.state_obj_load = LoadState::Init;
                }
            }
        }

        self.state_import.send_request(|obj, tx| {
            let rt = ctx.rt();
            let app = ctx.app().clone();
            let update_index = !self.skip_index;
            rt.spawn(async move {
                let res = import_object(&app, &obj, update_index).await;
                _ = tx.send(res)
            });
        });

        match self.state_import.get_response() {
            Some(Ok(())) => self.reset(),
            Some(Err(e)) => ctx.raise(e),
            None => {}
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut impl ErrorPopup) {
        egui::Window::new("Import")
            .open(&mut self.open)
            .constrain(true)
            .resizable(true)
            .resize(|r| r.max_size(egui::vec2(800.0, 600.0)))
            .show(ui.ctx(), |ui| {
                ui.heading("Import Location");
                ui.horizontal(|ui| {
                    egui::ScrollArea::horizontal()
                        .id_source("import input")
                        .auto_shrink([true, true])
                        .max_width(ui.available_width())
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.src)
                                    .clip_text(false)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("URL or File"),
                            );
                        });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let can_load =
                        !self.src.is_empty() && matches!(self.state_obj_load, LoadState::Init);
                    if ui
                        .add_enabled(can_load, egui::Button::new("Load"))
                        .clicked()
                    {
                        self.state_obj_load = LoadState::Triggered;
                    }
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

                    if matches!(self.state_obj_load, LoadState::Waiting(_)) {
                        ui.add(egui::Spinner::new().size(16.0));
                    }
                });
                if let Some(obj) = &self.object {
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .auto_shrink([true, true])
                        .max_height(ui.available_height() - 30.0)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui_for_object(ui, obj);
                        });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let waiting = matches!(self.state_import.state(), OneshotState::Wait);
                        if ui
                            .add_enabled(!waiting, egui::Button::new("Import"))
                            .clicked()
                        {
                            self.state_import.set_request(obj.clone());
                        }
                        if waiting {
                            ui.add(egui::Spinner::new().size(16.0));
                        }
                    });
                }
                ui.allocate_space(ui.available_size());
            });
    }
}

fn ui_for_object(ui: &mut egui::Ui, obj: &ImportObject) {
    match obj {
        ImportObject::CorpusExport(c) => {
            ui.heading(&c.title);
            ui.add_space(10.0);

            let mut media_export = c
                .content
                .iter()
                .map(|m| m.data.metadata.to_string())
                .collect::<Vec<_>>();
            media_export.sort_unstable();
            ui.label(format!("Total Episodes: {}", media_export.len()));
            ui.add_space(5.0);

            egui::ScrollArea::vertical()
                .id_source("obj_scroll")
                .auto_shrink([false, true])
                // .max_height(ui.available_height() - 30.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for ep in media_export {
                        ui.label(ep);
                    }
                });
        }
    }
}

use anyhow::Context;
use app::transcode::MakeGifRequest;
use egui::{Color32, RichText};
use lucille_core::uuid::Uuid;

use super::LucilleCtx;
use crate::gui_app::{error_popup::ErrorChainLogLine, oneshot_state::OneshotManager, ErrorPopup};

pub enum ClipSource {
    None,
    Search { uuid: Uuid, range: (usize, usize) },
}

#[derive(Default, Clone, Copy, PartialEq, serde::Deserialize, serde::Serialize)]
enum DataFormat {
    Debug,
    Json,
    JsonPretty,
    #[default]
    Base64,
}

impl DataFormat {
    fn display_name(&self) -> &'static str {
        match self {
            DataFormat::Debug => "Debug",
            DataFormat::Json => "JSON",
            DataFormat::JsonPretty => "Pretty JSON",
            DataFormat::Base64 => "Base64",
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Default)]
pub struct GifCreationUi {
    render_url: String,
    format: DataFormat,
    #[serde(skip)]
    transcode_request: Option<MakeGifRequest>,
    #[serde(skip)]
    gif_request: OneshotManager<MakeGifRequest, String>,
    #[serde(skip)]
    gif_url: Option<String>,
}

async fn send_gif_request(req: reqwest::RequestBuilder) -> anyhow::Result<String> {
    let resp = req.send().await.context("request failed")?;
    let t = resp
        .text()
        .await
        .context("failure reading response bytes")?;
    Ok(t)
}

impl GifCreationUi {
    pub fn set_clip(&mut self, uuid: Uuid, range: (usize, usize)) {
        self.transcode_request = Some(MakeGifRequest {
            segments: vec![app::transcode::SubSegment {
                srt_uuid: uuid,
                sub_range: range.0..range.1,
            }],
        })
    }
    pub fn update(&mut self, ctx: &mut (impl LucilleCtx + ErrorPopup)) {
        self.gif_request.send_request(|req, tx| {
            let rt = ctx.rt();
            let http_client = reqwest::Client::new();
            let body = serde_json::to_vec(&req).unwrap();
            let r = http_client
                .post(&self.render_url)
                .header("content-type", "application/json")
                .body(body);
            rt.spawn(async move {
                let res = send_gif_request(r).await;
                _ = tx.send(res)
            });
        });
        match self.gif_request.get_response() {
            Some(Ok(obj)) => self.gif_url = Some(obj),
            Some(Err(e)) => ctx.raise(e),
            None => {}
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for format in &[
                DataFormat::Debug,
                DataFormat::Json,
                DataFormat::JsonPretty,
                DataFormat::Base64,
            ] {
                let mut rich = RichText::new(format.display_name());
                if self.format == *format {
                    rich = rich.background_color(Color32::from_gray(128));
                }
                if ui.button(rich).clicked() {
                    self.format = *format
                }
            }
        });
        let string_req = self.format_request();
        ui.horizontal(|ui| {
            let rich = RichText::new(string_req.as_deref().unwrap_or_default())
                .text_style(egui::style::TextStyle::Monospace);

            ui.add(egui::Label::new(rich).wrap(true));
        });

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(ui.available_height() - 30.0)
            .stick_to_bottom(false)
            .enable_scrolling(false)
            .show(ui, |ui| {
                ui.text_edit_singleline(&mut self.render_url);
                if let Some(url) = &self.gif_url {
                    if ui.link(url).clicked() {
                        ui.output_mut(|o| o.copied_text = url.clone());
                    }
                }
            });

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let copy_button =
                    ui.add_enabled(string_req.is_some(), egui::Button::new("Copy to Clipboard"));
                if copy_button.clicked() {
                    ui.output_mut(|o| {
                        o.copied_text = string_req.clone().unwrap();
                    });
                }
                let send_button = ui.add_enabled(
                    self.transcode_request.is_some() && !self.render_url.is_empty(),
                    egui::Button::new("Send Request"),
                );
                if send_button.clicked() {
                    self.gif_request
                        .set_request(self.transcode_request.clone().unwrap())
                }
                if self.gif_request.state().is_waiting() {
                    ui.add(egui::Spinner::new().size(16.0));
                }
            });
        });
    }

    fn format_request(&self) -> Option<String> {
        let req = self.transcode_request.as_ref()?;
        match self.format {
            DataFormat::Debug => Some(format!("{:#?}", req)),
            DataFormat::Json => match serde_json::to_string(req) {
                Ok(s) => Some(s),
                Err(e) => {
                    let e = anyhow::Error::from(e)
                        .context("unable to serialize json")
                        .context("unable to format MakeGifRequest");
                    log::error!("{:?}", ErrorChainLogLine::from(&e));
                    None
                }
            },
            DataFormat::JsonPretty => match serde_json::to_string_pretty(req) {
                Ok(s) => Some(s),
                Err(e) => {
                    let e = anyhow::Error::from(e)
                        .context("unable to serialize pretty json")
                        .context("unable to format MakeGifRequest");
                    log::error!("{:?}", ErrorChainLogLine::from(&e));
                    None
                }
            },
            DataFormat::Base64 => match serde_json::to_string(req) {
                Ok(s) => Some(lucille_core::base64::B64Bytes::from(s.as_str()).to_string()),
                Err(e) => {
                    let e = anyhow::Error::from(e)
                        .context("unable to serialize json")
                        .context("unable to format MakeGifRequest");
                    log::error!("{:?}", ErrorChainLogLine::from(&e));
                    None
                }
            },
        }
    }
}

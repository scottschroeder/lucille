use app::transcode::MakeGifRequest;
use egui::{Color32, RichText};
use lucille_core::uuid::Uuid;

use crate::gui_app::error_popup::ErrorChainLogLine;

pub enum ClipSource {
    None,
    Search { uuid: Uuid, range: (usize, usize) },
}

#[derive(Default, Clone, Copy, PartialEq)]
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

#[derive(Default)]
pub struct GifCreationUi {
    transcode_request: Option<MakeGifRequest>,
    format: DataFormat,
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
            .show(ui, |_ui| {});

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let copy_button =
                    ui.add_enabled(string_req.is_some(), egui::Button::new("Copy to Clipboard"));
                if copy_button.clicked() {
                    ui.output_mut(|o| {
                        o.copied_text = string_req.unwrap();
                    });
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

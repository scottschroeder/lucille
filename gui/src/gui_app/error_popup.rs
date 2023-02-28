use std::fmt;

use egui::RichText;

const ERROR_MANAGER_UNIQUE_ID_HASH: &str = "error_popup_manager";

pub trait ErrorPopup {
    fn raise(&mut self, err: anyhow::Error);
    fn handle_err<T>(&mut self, res: anyhow::Result<T>) -> Option<T> {
        match res {
            Ok(t) => Some(t),
            Err(e) => {
                self.raise(e);
                None
            }
        }
    }

    // fn popup(&mut self, title: impl Into<String>, text: impl Into<egui::WidgetText>) {

    // }
}

impl<'a, T> ErrorPopup for &'a mut T
where
    T: ErrorPopup,
{
    fn raise(&mut self, err: anyhow::Error) {
        (*self).raise(err)
    }
}

pub struct ErrorChainLogLine<'a> {
    inner: &'a anyhow::Error,
}

impl<'a> From<&'a anyhow::Error> for ErrorChainLogLine<'a> {
    fn from(e: &'a anyhow::Error) -> Self {
        ErrorChainLogLine { inner: e }
    }
}

impl<'a> fmt::Debug for ErrorChainLogLine<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;

        for ec in self.inner.chain() {
            if first {
                first = false;
            } else {
                write!(f, " -> ")?;
            }
            write!(f, "{}", ec)?
        }
        Ok(())
    }
}

pub struct ErrorUi {
    error: anyhow::Error,
}

impl From<anyhow::Error> for ErrorUi {
    fn from(e: anyhow::Error) -> Self {
        ErrorUi::new(e)
    }
}

impl ErrorUi {
    pub fn new(err: anyhow::Error) -> ErrorUi {
        ErrorUi { error: err }
    }

    pub fn get(&self) -> &anyhow::Error {
        &self.error
    }

    pub fn log_display(&self) -> ErrorChainLogLine<'_> {
        ErrorChainLogLine::from(self.get())
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut chain = self.error.chain();
        ui.horizontal(|ui| {
            if let Some(first) = chain.next() {
                let rich = RichText::new(format!("{}", first))
                    .text_style(egui::style::TextStyle::Monospace)
                    .strong();
                ui.label(rich);
            }
        });
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .max_height(ui.available_height() - 30.0)
            .stick_to_bottom(false)
            .show(ui, |ui| {
                for ec in chain {
                    let rich = RichText::new(format!("-> {}", ec))
                        .text_style(egui::style::TextStyle::Monospace);
                    ui.label(rich);
                }
            });

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Copy to Clipboard").clicked() {
                    ui.output_mut(|o| {
                        o.copied_text = format!("{:?}", self.log_display());
                    });
                }
            });
        });
    }
}

#[derive(Default)]
pub struct ErrorManager {
    inner: Vec<(bool, egui::Id, ErrorUi)>,
    id: usize,
}

impl ErrorPopup for ErrorManager {
    fn raise(&mut self, err: anyhow::Error) {
        let id = egui::Id::new((ERROR_MANAGER_UNIQUE_ID_HASH, self.id));
        self.id += 1;
        let eui = ErrorUi::from(err);
        log::error!("{:?}", eui.log_display());
        self.inner.push((true, id, eui));
    }
}

impl ErrorManager {
    pub fn show(&mut self, ctx: &egui::Context) {
        for (show, id, err_ui) in &mut self.inner {
            egui::Window::new("Error")
                .id(*id)
                .open(show)
                .show(ctx, |ui| {
                    err_ui.ui(ui);
                });
        }

        self.clear_seen();
    }

    fn clear_seen(&mut self) {
        self.inner.retain(|(show, _, _)| *show)
    }
}

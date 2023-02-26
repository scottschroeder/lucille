use crate::app::HotKeyManager;

use super::card::Card;
use egui::Ui;

pub(crate) enum ChoiceOutcome {
    Left,
    Right,
    Equal,
}

pub(crate) struct ChoicePane<'a> {
    pub(crate) card: Card<'a>,
}

impl<'a> ChoicePane<'a> {
    pub(crate) fn update(&self, ui: &mut Ui) -> bool {
        let mut clicked = false;
        ui.vertical_centered(|ui| {
            self.card.update(ui);
            clicked = ui.button("Choose").clicked();
        });
        clicked
    }
}

pub(crate) struct ChoiceSelection<'a> {
    pub(crate) lhs: ChoicePane<'a>,
    pub(crate) rhs: ChoicePane<'a>,
}

impl<'a> ChoiceSelection<'a> {
    pub(crate) fn update(&self, ui: &mut Ui, hotkeys: &mut HotKeyManager) -> Option<ChoiceOutcome> {
        let mut clicked = None;
        ui.vertical_centered(|ui| {
            ui.columns(2, |columns| {
                if self.lhs.update(&mut columns[0]) {
                    clicked = Some(ChoiceOutcome::Left);
                }
                if self.rhs.update(&mut columns[1]) {
                    clicked = Some(ChoiceOutcome::Right);
                }
            });
            if ui.button("About Equal").clicked() {
                clicked = Some(ChoiceOutcome::Equal);
            }
        });

        if hotkeys.key_pressed(ui, egui::Key::ArrowLeft) {
            clicked = Some(ChoiceOutcome::Left);
        } else if hotkeys.key_pressed(ui, egui::Key::ArrowRight) {
            clicked = Some(ChoiceOutcome::Right);
        } else if hotkeys.key_pressed(ui, egui::Key::ArrowDown) {
            clicked = Some(ChoiceOutcome::Equal);
        }

        clicked
    }
}

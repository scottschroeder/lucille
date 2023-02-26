#[derive(Default)]
pub(crate) struct HotKeyManager {
    cleared: bool,
}

impl HotKeyManager {
    pub fn check_cleared(&mut self, ui: &mut egui::Ui) {
        if !self.cleared && ui.input().keys_down.is_empty() {
            self.cleared = true;
        }
    }

    pub fn key_pressed(&mut self, ui: &mut egui::Ui, key: egui::Key) -> bool {
        if !self.cleared {
            return false;
        }
        if ui.input().key_pressed(key) {
            self.cleared = false;
            true
        } else {
            false
        }
    }
}

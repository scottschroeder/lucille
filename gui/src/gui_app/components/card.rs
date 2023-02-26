use crate::app::{siftimage::SiftImage, NamedImage};
use std::borrow::Cow;

type Text<'a> = Cow<'a, str>;

pub trait CardDisplay {
    fn make_card(&self) -> Card<'_>;
}

pub enum Card<'a> {
    TextCard(TextCard<'a>),
    ImageTextCard(ImageTextCard<'a>),
}

impl<'a> Card<'a> {
    pub(crate) fn update(&self, ui: &mut egui::Ui) {
        match self {
            Card::TextCard(t) => t.update(ui),
            Card::ImageTextCard(img) => img.update(ui),
        }
    }
    pub(crate) fn update_short(&self, ui: &mut egui::Ui) {
        match self {
            Card::TextCard(t) => t.update(ui),
            Card::ImageTextCard(t) => t.to_text().update(ui),
        }
    }
}

pub struct TextCard<'a> {
    pub(crate) data: Text<'a>,
}

impl<'a> TextCard<'a> {
    pub(crate) fn update(&self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(self.data.as_ref()));
    }
}

pub struct ImageTextCard<'a> {
    pub(crate) data: Text<'a>,
    pub(crate) image: &'a SiftImage,
}

impl<'a> ImageTextCard<'a> {
    pub(crate) fn update(&self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let scaled = egui::Vec2 {
            x: size.x * 0.8,
            y: size.y * 0.8,
        };

        self.image.image.show_max_size(ui, scaled);
        ui.label(egui::RichText::new(self.data.as_ref()));
    }
    fn to_text(&'a self) -> TextCard<'a> {
        TextCard {
            data: Cow::from(self.data.as_ref()),
        }
    }
}

impl CardDisplay for usize {
    fn make_card(&self) -> Card<'_> {
        Card::TextCard(TextCard {
            data: Cow::from(format!("{}", self)),
        })
    }
}

impl CardDisplay for NamedImage {
    fn make_card(&self) -> Card<'_> {
        Card::ImageTextCard(ImageTextCard {
            data: Cow::from(self.entry.name.as_str()),
            image: &self.image,
        })
    }
}

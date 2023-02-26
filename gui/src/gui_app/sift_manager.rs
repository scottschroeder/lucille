use super::{
    database::{Corpus, CorpusType, Item},
    image_sorter::{load_app_from_db_realz, update_db_from_directory, NamedImage},
    sift_app::SiftApp,
    AppCtx,
};
use rand::prelude::{thread_rng, SliceRandom};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_LAST_SIZE: usize = 10;

enum SupportedApps {
    Image(SiftApp<NamedImage>),
    Numeric(SiftApp<usize>),
}

#[derive(Default, Serialize, Deserialize)]
pub struct SiftManager {
    #[serde(skip)]
    sift: Option<SupportedApps>,
    #[serde(default = "default_last_size")]
    last_size: usize,
}

fn default_last_size() -> usize {
    DEFAULT_LAST_SIZE
}

impl SiftManager {
    pub(crate) fn update_side_panel(&mut self, ui: &mut egui::Ui) {
        match &mut self.sift {
            Some(sift) => {
                ui.heading("Current Order");
                match sift {
                    SupportedApps::Image(app) => app.update_order(ui),
                    SupportedApps::Numeric(app) => app.update_order(ui),
                }
            }
            None => {}
        }
    }
    pub(crate) fn update_central_panel(&mut self, ui: &mut egui::Ui, app_ctx: &mut AppCtx<'_>) {
        match &mut self.sift {
            Some(sift) => {
                ui.heading("Choose Your Favorite");
                match sift {
                    SupportedApps::Image(app) => app.update(ui, app_ctx),
                    SupportedApps::Numeric(app) => app.update(ui, app_ctx),
                }
            }
            None => {
                let mut loader = SiftLoader {
                    last_size: &mut self.last_size,
                };
                if let Some(selection) = loader.update_central_panel(ui, app_ctx).unwrap() {
                    self.load(selection, app_ctx).unwrap();
                }
            }
        }
    }

    pub fn unload(&mut self) {
        self.sift = None;
    }

    pub fn is_loaded(&self) -> bool {
        self.sift.is_some()
    }

    fn load_scores(&mut self, ctx: &mut AppCtx<'_>) -> anyhow::Result<()> {
        if let Some(sift) = &mut self.sift {
            match sift {
                SupportedApps::Image(app) => app.load_scores(ctx),
                SupportedApps::Numeric(app) => app.load_scores(ctx),
            }
        } else {
            Ok(())
        }
    }

    fn load(&mut self, selection: LoaderSelection, ctx: &mut AppCtx<'_>) -> anyhow::Result<()> {
        let corpus_name = selection.corpus_name();
        match selection {
            LoaderSelection::Numeric(s) => {
                let corpus = ctx.db.new_corpus(corpus_name, CorpusType::Numeric)?;
                let mut rng = thread_rng();
                let mut v = (0..s).collect::<Vec<_>>();
                v.shuffle(&mut rng);
                ctx.db.add_items(&corpus, &v)?;
                let items: Vec<Item<usize>> = ctx.db.get_items(&corpus)?;
                self.last_size = s;
                self.sift = Some(SupportedApps::Numeric(SiftApp::new(corpus, items)?));
            }
            LoaderSelection::Path(selected_path) => {
                let corpus = ctx.db.new_corpus(corpus_name, CorpusType::ReferenceImage)?;
                update_db_from_directory(ctx.db, &corpus, selected_path.as_path())?;
                let app = load_app_from_db_realz(ctx.db, corpus)?;

                self.sift = Some(SupportedApps::Image(app));
            }
            LoaderSelection::Existing(c) => match c.corpus_type {
                CorpusType::Numeric => {
                    let items: Vec<Item<usize>> = ctx.db.get_items(&c)?;
                    self.sift = Some(SupportedApps::Numeric(SiftApp::new(c, items)?));
                }
                CorpusType::ReferenceImage => {
                    let app = load_app_from_db_realz(ctx.db, c)?;
                    self.sift = Some(SupportedApps::Image(app));
                }
            },
        }
        self.load_scores(ctx)
    }
}

struct SiftLoader<'a> {
    last_size: &'a mut usize,
}

enum LoaderSelection {
    Numeric(usize),
    Path(PathBuf),
    Existing(Corpus),
}

impl LoaderSelection {
    fn corpus_name(&self) -> String {
        match self {
            LoaderSelection::Numeric(s) => format!("Numbers [{}]", s),
            LoaderSelection::Path(p) => format!("Image Directory [{:?}]", p),
            LoaderSelection::Existing(c) => c.name.clone(),
        }
    }
}

impl<'a> SiftLoader<'a> {
    fn update_central_panel(
        &mut self,
        ui: &mut egui::Ui,
        app_ctx: &mut AppCtx<'_>,
    ) -> anyhow::Result<Option<LoaderSelection>> {
        let SiftLoader { last_size } = self;
        let text_style = egui::TextStyle::Body;
        let row_height = ui.text_style_height(&text_style) * 3.0;
        let mut selection = None;

        ui.heading("Create a new Dataset");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(*last_size)
                    .speed(1)
                    .clamp_range(2..=usize::MAX)
                    .prefix("items: "),
            );
            if ui.button("create numbers").clicked() {
                selection = Some(LoaderSelection::Numeric(**last_size));
            }

            if ui.button("Open a New Directory").clicked() {
                if let Some(p) = rfd::FileDialog::new().pick_folder() {
                    selection = Some(LoaderSelection::Path(p));
                }
            }
        });

        let existing = app_ctx.db.get_corpuses()?;

        ui.heading("Open an existing Dataset");
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show_rows(ui, row_height, existing.len(), |ui, row_range| {
                for row in row_range {
                    let c = &existing[row];
                    if ui.button(format!("{}", CorpusDisplay(c))).clicked() {
                        selection = Some(LoaderSelection::Existing(c.clone()));
                    }
                }
            });

        if let Some(s) = &selection {
            match s {
                LoaderSelection::Existing(_) => {}
                _ => {
                    let name = s.corpus_name();
                    if let Some(c) = existing.iter().find_map(|c| {
                        if c.name == name {
                            Some(c.clone())
                        } else {
                            None
                        }
                    }) {
                        selection = Some(LoaderSelection::Existing(c))
                    }
                }
            }
        }

        Ok(selection)
    }
}

struct CorpusDisplay<'a>(&'a Corpus);

impl<'a> std::fmt::Display for CorpusDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.0.corpus_type, self.0.name)
    }
}

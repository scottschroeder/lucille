use std::fmt::Write;

use anyhow::Context;
use app::{
    app::LucilleApp,
    search_manager::{ClipResult, SearchRequest, SearchResponse, SearchService},
};
use egui::RichText;
use lucille_core::{clean_sub::CleanSubs, uuid::Uuid};

use self::episode_cache::{EpisodeCache, EpisodeData};
use super::{super::error::ErrorChainLogLine, gif_creation::GifCreationUi, LucilleCtx};
use crate::gui_app::ErrorPopup;
mod loader;
pub(crate) use loader::{load_last_index, SearchAppState};

const DEFAULT_SEARCH_WIDTH: usize = 5;
const SEARCH_BUFFER_DEPTH: usize = 32;

type TxSend = tokio::sync::mpsc::Sender<SearchResults>;
type TxRecv = tokio::sync::mpsc::Receiver<SearchResults>;

mod episode_cache;

pub struct SearchResults {
    inner: SearchResponse,
    selected: Option<usize>,
    clip: ClipSelection,
}

#[derive(Default)]
struct ClipSelection {
    clip: Option<(usize, usize)>,
    open: bool,
}
impl ClipSelection {
    fn update_click(&mut self, idx: usize) {
        match (self.clip, self.open) {
            (None, _) => {
                self.clip = Some((idx, idx));
                self.open = true;
            }
            (Some((s, e)), true) => {
                if idx > e {
                    self.clip = Some((s, idx));
                } else if idx < s {
                    self.clip = Some((idx, e));
                } else if idx == s && idx == e {
                    return;
                }
                self.open = false;
            }
            (Some(_), false) => self.clip = None,
        }
    }

    fn check(&self, row: usize) -> bool {
        self.clip
            .map(|(s, e)| row >= s && row <= e)
            .unwrap_or(false)
    }
}

struct Lookback<T> {
    inner: Vec<T>,
    cursor: usize,
    size: usize,
}

impl<T> Lookback<T> {
    fn new(size: usize) -> Lookback<T> {
        Lookback {
            inner: Vec::with_capacity(size),
            cursor: 0,
            size,
        }
    }

    fn push(&mut self, item: T) {
        if self.inner.len() < self.size {
            self.inner.push(item);
        } else {
            self.inner[self.cursor % self.size] = item;
        }
        self.cursor += 1;
    }
    fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        let cursor = self.cursor - self.inner.len();
        (0..self.inner.len()).map(move |idx| &self.inner[(cursor + idx) % self.size])
    }
}

struct PreviewRow<'a> {
    clip: &'a ClipResult,
}

impl<'a> PreviewRow<'a> {
    fn display_text(&self, cache: &EpisodeCache) -> String {
        let mut text = String::new();
        let episode = cache.episode(self.clip.srt_id);
        let episode = episode.value();

        writeln!(&mut text, "{:?} {}", self.clip.score, episode.metadata).unwrap();

        let base = self.clip.offset;

        let mut lookback = Lookback::new(2);

        let mut emit_max = 5;
        let mut emit = |offset: usize| {
            if emit_max > 0 {
                emit_max -= 1;
                let script = CleanSubs(&episode.subs[base + offset..base + offset + 1]);
                writeln!(&mut text, "{}", script).unwrap();
            }
        };
        let mut start_display = false;
        for (offset, linescore) in self.clip.lines.iter().enumerate() {
            if linescore.score == self.clip.score {
                start_display = true;
                for prev in lookback.iter() {
                    emit(*prev);
                }
            } else if !start_display {
                lookback.push(offset)
            }
            if start_display {
                emit(offset)
            }
        }
        writeln!(&mut text).unwrap();
        text
    }
}

impl SearchResults {
    fn update_results_scroller(&mut self, cache: &EpisodeCache, ui: &mut egui::Ui) -> bool {
        let text_style = egui::TextStyle::Body;
        let row_height = ui.text_style_height(&text_style) * 7.0;
        let num_rows = self.inner.results.len();
        let mut clicked = false;
        egui::ScrollArea::vertical()
            .id_source("results_list")
            .auto_shrink([false; 2])
            .show_rows(ui, row_height, num_rows, |ui, row_range| {
                for row in row_range {
                    let preview = PreviewRow {
                        clip: &self.inner.results[row],
                    };
                    let text = preview.display_text(cache);
                    if ui.button(text).clicked() {
                        self.selected = Some(row);
                        clicked = true;
                    }
                }
            });
        clicked
    }
    fn update_results_details(
        &mut self,
        cache: &EpisodeCache,
        ui: &mut egui::Ui,
        scroll_force: bool,
    ) {
        let clip = match self.selected {
            Some(s) => &self.inner.results[s],
            None => return,
        };
        let episode = cache.episode(clip.srt_id);
        let episode = episode.value();

        let start_id = clip.offset;
        let end_id = clip.offset + clip.lines.len();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .id_source("results_details")
            .show(ui, |ui| {
                for row in 0..episode.subs.len() {
                    let cs = CleanSubs(&episode.subs[row..row + 1]);
                    let in_search = row < start_id || row >= end_id;
                    let sub_line = SubLine {
                        idx: row,
                        text: format!("{}", cs),
                        in_search,
                        selected: self.clip.check(row),
                    };
                    let resp = sub_line.ui(ui);
                    if resp.inner {
                        self.clip.update_click(row);
                    }
                    if scroll_force && row == start_id {
                        resp.response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
    }
    fn update(&mut self, cache: &EpisodeCache, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.columns(2, |columns| {
                let update = self.update_results_scroller(cache, &mut columns[0]);
                self.update_results_details(cache, &mut columns[1], update);
            });
        });
    }
}

#[derive(Debug)]
struct SubLine {
    idx: usize,
    text: String,
    in_search: bool,
    selected: bool,
}

impl SubLine {
    fn ui(&self, ui: &mut egui::Ui) -> egui::InnerResponse<bool> {
        let b = ui.button("");

        let mut bui = ui.child_ui(b.rect, egui::Layout::left_to_right(egui::Align::Center));
        bui.horizontal(|ui| {
            let text = RichText::new(&self.text);
            let text = if self.in_search { text.weak() } else { text };
            let size = egui::Vec2::splat(16.0);
            let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
            let rect = response.rect;
            let c = rect.center();
            let r = rect.width() / 2.0 - 1.0;
            let color = egui::Color32::from_gray(128);
            let stroke = egui::Stroke::new(1.0, color);
            if self.selected {
                painter.circle_filled(c, r, egui::Color32::BLUE);
            } else {
                painter.circle_stroke(c, r, stroke);
            }
            ui.label(text);
            let clicked = b.clicked();
            if clicked {
                log::debug!("clicked: {:?}", self);
            }
            clicked
        })
    }
}

pub struct SearchQuery {
    body: String,
}

pub struct SearchApp {
    pub search_service: std::sync::Arc<SearchService>,
    pub search_width: usize,
    pub query: SearchQuery,
    pub results: Option<SearchResults>,

    pub show_gif_creator: bool,

    cache: EpisodeCache,

    tx: TxSend,
    rx: TxRecv,
}

impl SearchApp {
    pub fn new(search_service: SearchService) -> SearchApp {
        let (tx, rx) = tokio::sync::mpsc::channel(SEARCH_BUFFER_DEPTH);
        SearchApp {
            search_service: std::sync::Arc::new(search_service),
            search_width: DEFAULT_SEARCH_WIDTH,
            query: SearchQuery {
                body: "".to_string(),
            },

            cache: EpisodeCache::default(),
            results: None,
            tx,
            rx,
            show_gif_creator: false,
        }
    }

    fn run_query<Ctx: ErrorPopup + LucilleCtx>(&self, ui: &mut egui::Ui, ctx: &mut Ctx) {
        let results_tx = self.tx.clone();
        let service = self.search_service.clone();
        let text = self.query.body.clone();
        let lucille = ctx.app().clone();
        let egui_ctx = ui.ctx().clone();
        let cache = self.cache.clone();
        ctx.rt().spawn(async move {
            let request = SearchRequest {
                query: text.as_str(),
                window: Some(DEFAULT_SEARCH_WIDTH),
                max_responses: Some(100),
            };
            if let Err(e) = search_and_rank(&lucille, &service, &cache, request, results_tx).await {
                log::error!("{:?}", ErrorChainLogLine::from(e))
            } else {
                egui_ctx.request_repaint();
            }
        });
    }

    fn fetch_latest_result(&mut self) {
        let mut new_results = None;
        while let Ok(r) = self.rx.try_recv() {
            new_results = Some(r)
        }
        if let Some(new_results) = new_results {
            self.results = Some(new_results)
        }
    }

    pub(crate) fn update_central_panel<Ctx>(
        &mut self,
        ui: &mut egui::Ui,
        app_ctx: &mut Ctx,
        gif: &mut GifCreationUi,
    ) where
        Ctx: ErrorPopup + LucilleCtx,
    {
        ui.vertical(|ui| {
            ui.heading("Search");
            if ui.text_edit_singleline(&mut self.query.body).changed() {
                self.run_query(ui, app_ctx)
            }
            self.fetch_latest_result();
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(ui.available_height() - 30.0)
                .enable_scrolling(false)
                .show(ui, |ui| {
                    if let Some(results) = &mut self.results {
                        results.update(&self.cache, ui)
                    }
                });
            ui.with_layout(egui::Layout::bottom_up(egui::Align::BOTTOM), |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let enable_make_gif = self
                        .results
                        .as_ref()
                        .map(|r| r.clip.clip.is_some())
                        .unwrap_or(false);

                    let create = ui.add_enabled(enable_make_gif, egui::Button::new("Create GIF"));
                    if create.clicked() {
                        self.show_gif_creator = true
                    }
                });
            });
        });
        gif.update(app_ctx);
        if let Some((uuid, range)) = self.get_clip_ids() {
            gif.set_clip(uuid, range);
        }
        egui::Window::new("Gif Creation")
            .open(&mut self.show_gif_creator)
            .show(ui.ctx(), |ui| {
                gif.ui(ui);
            });
    }

    fn get_clip_ids(&self) -> Option<(Uuid, (usize, usize))> {
        let result = self.results.as_ref()?;
        let result_idx = result.selected?;
        let clip_result = result.inner.results.get(result_idx)?;
        let range = result.clip.clip?;
        let episode = self.cache.episode(clip_result.srt_id);
        let uuid = episode.value().uuid;
        Some((uuid, range))
    }
}

async fn search_and_rank<'a>(
    app: &LucilleApp,
    search: &SearchService,
    cache: &EpisodeCache,
    req: SearchRequest<'a>,
    tx: TxSend,
) -> anyhow::Result<()> {
    let resp = search
        .search_and_rank(req)
        .await
        .context("SearchService search and rank failure")?;
    let results = fill_cache_with_results(app, resp, cache)
        .await
        .context("convert resp to display results")?;
    tx.send(results)
        .await
        .map_err(|e| anyhow::anyhow!("tokio send failure: {}", e))
        .context("send results back to gui thread")?;
    Ok(())
}

async fn fill_cache_with_results(
    app: &LucilleApp,
    resp: SearchResponse,
    cache: &EpisodeCache,
) -> anyhow::Result<SearchResults> {
    for clip in &resp.results {
        if !cache.contains(clip.srt_id) {
            let (_, metadata) = app.db.get_episode_by_id(clip.srt_id).await?;
            let subs = app.db.get_all_subs_for_srt(clip.srt_id).await?;
            let uuid = app.db.get_srt_uuid_by_id(clip.srt_id).await?;
            let e = EpisodeData {
                uuid,
                metadata,
                subs,
            };
            cache.insert(clip.srt_id, e);
        }
    }

    Ok(SearchResults {
        inner: resp,
        selected: None,
        clip: ClipSelection::default(),
    })
}

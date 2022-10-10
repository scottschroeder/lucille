use anyhow::Context;
use app::{
    app::LucileApp,
    search_manager::{ClipResult, SearchRequest, SearchResponse, SearchService},
};
use egui::{Button, RichText};
use lucile_core::{clean_sub::CleanSubs, metadata::MediaMetadata, Subtitle};
use std::fmt::Write;

use self::episode_cache::{EpisodeCache, EpisodeData};

use super::{error::ErrorChainLogLine, AppCtx};

const DEFAULT_SEARCH_WIDTH: usize = 5;
const SEARCH_BUFFER_DEPTH: usize = 32;

type TxSend = tokio::sync::mpsc::Sender<SearchResults>;
type TxRecv = tokio::sync::mpsc::Receiver<SearchResults>;

mod episode_cache;

pub struct SearchResults {
    inner: SearchResponse,
    selected: Option<usize>,
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

        writeln!(&mut text, "{:?} {}", self.clip.score, episode.metadata);

        let base = self.clip.offset;

        let mut lookback = Lookback::new(2);

        let mut emit_max = 5;
        let mut emit = |offset: usize| {
            if emit_max > 0 {
                emit_max -= 1;
                let script = CleanSubs(&episode.subs[base + offset..base + offset + 1]);
                writeln!(&mut text, "{}", script);
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
        writeln!(&mut text);
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
                    let text = RichText::new(format!("{}", cs));
                    let text = if row < start_id || row >= end_id {
                        text.weak()
                    } else {
                        text
                    };
                    let bttn = ui.button(text);
                    if scroll_force && row == start_id {
                        bttn.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
    }
    fn update(&mut self, cache: &EpisodeCache, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.columns(2, |columns| {
                let update = self.update_results_scroller(cache, &mut columns[0]);
                self.update_results_details(cache, &mut columns[1], update);
            })
        });
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
        }
    }

    fn run_query(&self, ui: &mut egui::Ui, ctx: &mut AppCtx) {
        let results_tx = self.tx.clone();
        let service = self.search_service.clone();
        let text = self.query.body.clone();
        let lucile = ctx.lucile.clone();
        let egui_ctx = ui.ctx().clone();
        let cache = self.cache.clone();
        ctx.rt.spawn(async move {
            let request = SearchRequest {
                query: text.as_str(),
                window: Some(DEFAULT_SEARCH_WIDTH),
                max_responses: Some(100),
            };
            if let Err(e) = search_and_rank(&lucile, &service, &cache, request, results_tx).await {
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

    pub(crate) fn update_central_panel(&mut self, ui: &mut egui::Ui, ctx: &mut AppCtx) {
        ui.heading("Search");
        if ui.text_edit_singleline(&mut self.query.body).changed() {
            self.run_query(ui, ctx)
        }
        self.fetch_latest_result();
        if let Some(results) = &mut self.results {
            results.update(&self.cache, ui)
        }
    }
}

async fn search_and_rank<'a>(
    app: &LucileApp,
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
    app: &LucileApp,
    resp: SearchResponse,
    cache: &EpisodeCache,
) -> anyhow::Result<SearchResults> {
    for clip in &resp.results {
        if !cache.contains(clip.srt_id) {
            let (_, metadata) = app.db.get_episode_by_id(clip.srt_id).await?;
            let subs = app.db.get_all_subs_for_srt(clip.srt_id).await?;
            let e = EpisodeData { metadata, subs };
            cache.insert(clip.srt_id, e);
        }
    }

    Ok(SearchResults {
        inner: resp,
        selected: None,
    })
}

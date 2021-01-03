use anyhow::Result;
use tantivy::Index;
use uuid::Uuid;

use crate::{content::Content, srt_loader::CleanSub};

const DEFAULT_WINDOW: usize = 5;
const DEFAULT_MAX_RESPONSES: usize = 5;

pub trait SearchClient {
    fn search<'r>(&self, req: SearchRequest<'r>) -> Result<SearchResponse>;
}

impl<'a> SearchClient for SearchService<'a> {
    fn search<'r>(&self, req: SearchRequest<'r>) -> Result<SearchResponse> {
        self.search_and_rank(req)
    }
}

pub struct SearchRequest<'a> {
    pub query: &'a str,
    pub window: Option<usize>,
    pub max_responses: Option<usize>,
}

impl<'a> SearchRequest<'a> {
    fn get_window(&self) -> usize {
        self.window.unwrap_or(DEFAULT_WINDOW)
    }
    fn get_max_responses(&self) -> usize {
        self.max_responses.unwrap_or(DEFAULT_MAX_RESPONSES)
    }
}

pub struct ClipResult {
    pub episode_id: usize,
    pub offset: usize,
    pub title: String,
    pub score: f32,
    pub lines: Vec<LineScore>,
}

pub struct LineScore {
    pub score: f32,
    pub text: String,
}

pub struct SearchResponse {
    pub index: Uuid,
    pub results: Vec<ClipResult>,
}

pub struct SearchService<'a> {
    id: Uuid,
    index: Index,
    content: &'a Content,
}

impl<'a> SearchService<'a> {
    pub fn new(id: Uuid, index: Index, content: &'a Content) -> SearchService {
        SearchService { id, index, content }
    }

    pub fn search_and_rank<'r>(&self, request: SearchRequest<'r>) -> Result<SearchResponse> {
        let scores = crate::search::search(&self.index, request.query, request.get_window())
            .map_err(crate::error::TError::from)?;
        let results = crate::search::rank(&scores, request.get_max_responses())
            .into_iter()
            .map(|rm| {
                let episode_id = rm.ep;
                let offset = rm.clip.index;
                let episode = &self.content.episodes[episode_id];
                let lines = rm
                    .clip
                    .scores
                    .iter()
                    .zip(episode.subtitles.iter().skip(offset))
                    .map(|(score, sub)| LineScore {
                        score: score.0,
                        text: format!("{}", CleanSub(sub)),
                    })
                    .collect::<Vec<_>>();
                ClipResult {
                    episode_id,
                    offset,
                    title: episode.title.clone(),
                    score: rm.score.0,
                    lines,
                }
            })
            .collect::<Vec<_>>();
        Ok(SearchResponse {
            index: self.id,
            results,
        })
    }
}

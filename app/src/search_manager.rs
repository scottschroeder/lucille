use lucile_core::uuid::Uuid;
use search::SearchIndex;
use serde::{Deserialize, Serialize};

use crate::LucileAppError;

const DEFAULT_WINDOW: usize = 5;
const DEFAULT_MAX_RESPONSES: usize = 5;

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct ClipResult {
    pub srt_id: i64,
    pub offset: usize,
    pub score: f32,
    pub lines: Vec<LineScore>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LineScore {
    pub score: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub index: Uuid,
    pub results: Vec<ClipResult>,
}

pub struct SearchService {
    pub(crate) index: SearchIndex,
}

impl SearchService {
    pub fn new(index: SearchIndex) -> SearchService {
        SearchService { index }
    }
    pub async fn search_and_rank<'r>(
        &self,
        request: SearchRequest<'r>,
    ) -> Result<SearchResponse, LucileAppError> {
        let scores = self.index.search(request.query, request.get_window())?;

        // TODO what does this actually do? nothing? I think its nothing...
        // it reverses a list
        //
        //
        let mut results = Vec::new();
        for rm in search::rank(&scores)
            .into_iter()
            .rev()
            .take(request.get_max_responses())
        {
            let srt_id = rm.ep as i64;
            let offset = rm.clip.index;
            let lines = rm
                .clip
                .scores
                .iter()
                .map(|score| LineScore { score: score.0 })
                .collect::<Vec<_>>();
            results.push(ClipResult {
                srt_id,
                offset,
                score: rm.score.0,
                lines,
            })
        }
        Ok(SearchResponse {
            index: self.index.uuid(),
            results,
        })
    }
}

use lucile_core::{clean_sub::CleanSub, metadata::MediaHash, uuid::Uuid};
use search::SearchIndex;
use serde::{Deserialize, Serialize};

use crate::{app::LucileApp, LucileAppError};

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
    pub media_hash: MediaHash,
    pub srt_id: i64,
    pub offset: usize,
    pub title: String,
    pub score: f32,
    pub lines: Vec<LineScore>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LineScore {
    pub score: f32,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub index: Uuid,
    pub results: Vec<ClipResult>,
}

pub struct SearchService<'a> {
    pub(crate) index: SearchIndex,
    pub(crate) app: &'a LucileApp,
}

impl<'a> SearchService<'a> {
    pub fn new(index: SearchIndex, app: &'a LucileApp) -> SearchService {
        SearchService { index, app }
    }
    pub async fn search_and_rank<'r>(
        &self,
        request: SearchRequest<'r>,
    ) -> Result<SearchResponse, LucileAppError> {
        let scores = self.index.search(request.query, request.get_window())?;

        let mut results = Vec::new();
        for rm in search::rank(&scores)
            .into_iter()
            .rev()
            .take(request.get_max_responses())
        {
            let srt_id = rm.ep as i64;
            let offset = rm.clip.index;
            let (hash, metadata) = self.app.db.get_episode_by_id(srt_id).await?;
            let srt = self.app.db.get_all_subs_for_srt(srt_id).await?;
            let lines = rm
                .clip
                .scores
                .iter()
                .zip(srt.iter().skip(offset))
                .map(|(score, sub)| LineScore {
                    score: score.0,
                    text: format!("{}", CleanSub(sub)),
                })
                .collect::<Vec<_>>();
            results.push(ClipResult {
                media_hash: hash,
                srt_id,
                offset,
                title: metadata.title(),
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

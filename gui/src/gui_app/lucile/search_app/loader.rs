use anyhow::Context;
use app::app::LucileApp;

use super::SearchApp;
use crate::gui_app::{lucile::LucileCtx, ErrorPopup};

#[derive(Default)]
pub(crate) enum SearchAppState {
    #[default]
    Unknown,
    None,
    App(SearchApp),
}

async fn load_search(lucile: &LucileApp) -> anyhow::Result<Option<SearchApp>> {
    log::trace!("looking up most recent search indicies");
    let existing_indexes = lucile
        .db
        .get_search_indexes()
        .await
        .context("could not load previous search index list")?;

    for id in existing_indexes.iter().rev() {
        let s = match lucile.search_service(*id) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("unable to load search service {}: {}", id, e);
                continue;
            }
        };
        return Ok(Some(SearchApp::new(s)));
    }
    Ok(None)
}

pub(crate) fn load_last_index<Ctx: ErrorPopup + LucileCtx>(ctx: &mut Ctx) -> SearchAppState {
    let lucile = ctx.app().clone();
    match ctx
        .rt()
        .block_on(async { load_search(&lucile).await })
        .context("unable to run import/index in background thread")
    {
        Ok(o) => match o {
            Some(s) => SearchAppState::App(s),
            None => SearchAppState::None,
        },
        Err(e) => {
            ctx.raise(e);
            SearchAppState::None
        }
    }
}

use anyhow::Context;
use app::app::LucilleApp;

use super::SearchApp;
use crate::gui_app::{lucille::LucilleCtx, ErrorPopup};

#[derive(Default)]
pub(crate) enum SearchAppState {
    #[default]
    Unknown,
    None,
    App(SearchApp),
}

async fn load_search(lucille: &LucilleApp) -> anyhow::Result<Option<SearchApp>> {
    log::trace!("looking up most recent search indicies");
    let existing_indexes = lucille
        .db
        .get_search_indexes()
        .await
        .context("could not load previous search index list")?;

    for id in existing_indexes.iter().rev() {
        let s = match lucille.search_service(*id) {
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

pub(crate) fn load_last_index<Ctx: ErrorPopup + LucilleCtx>(ctx: &mut Ctx) -> SearchAppState {
    let lucille = ctx.app().clone();
    match ctx
        .rt()
        .block_on(async { load_search(&lucille).await })
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

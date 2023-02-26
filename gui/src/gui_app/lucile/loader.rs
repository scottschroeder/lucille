use anyhow::Context;
use app::app::LucileApp;

use super::{AppCtx, SearchApp};
use crate::gui_app::error::ErrorChainLogLine;

enum LoadManager<T> {
    Init,
    Unloaded,
    Error(Option<anyhow::Error>),
    Ready(T),
}

impl<T> Default for LoadManager<T> {
    fn default() -> Self {
        LoadManager::Init
    }
}

impl<T> LoadManager<T> {
    pub fn reset(&mut self) {
        *self = LoadManager::Unloaded
    }

    pub fn get(&self) -> Option<&T> {
        match self {
            LoadManager::Ready(t) => Some(t),
            _ => None,
        }
    }
}

enum LoadState {
    None,
    App(std::sync::Arc<LucileApp>),
    Ready((std::sync::Arc<LucileApp>, SearchApp)),
}
//
pub(crate) enum SearchAppState {
    Unknown,
    None,
    App(SearchApp),
}

impl LoadState {
    fn add_search(&mut self, search_app: SearchApp) {
        let mut swp = LoadState::None;
        std::mem::swap(self, &mut swp);
        let mut swp = if let LoadState::App(app) = swp {
            LoadState::Ready((app, search_app))
        } else {
            panic!("dont add search to anything but a loaded app state");
        };
        std::mem::swap(self, &mut swp);
    }
}

pub(crate) struct LoadedShell {
    pub(crate) rt: tokio::runtime::Runtime,
    pub(crate) lucile: std::sync::Arc<LucileApp>,
}

async fn load_search(lucile: &LucileApp) -> anyhow::Result<Option<SearchApp>> {
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

pub(crate) fn load_last_index(ctx: &mut AppCtx<'_>) -> SearchAppState {
    let lucile = ctx.lucile.clone();
    match ctx
        .rt
        .block_on(async { load_search(&lucile).await })
        .context("unable to run import/index in background thread")
    {
        Ok(o) => match o {
            Some(s) => SearchAppState::App(s),
            None => SearchAppState::None,
        },
        Err(e) => {
            log::error!("{:?}", ErrorChainLogLine::from(e));
            SearchAppState::None
        }
    }
}

// async fn load_app_with_search(state: LoadState) -> anyhow::Result<LoadState> {
//     match state {
//         LoadState::None => {
//             let app = LucileApp::create::<&str, &str>(None, None)
//                 .await
//                 .context("could not load lucile app")?;
//             LoadState::App(std::sync::Arc::new(app))
//         }
//         LoadState::App(app) => {
//             let existing_indexes = app
//                 .db
//                 .get_search_indexes()
//                 .await
//                 .context("could not load previous search index list")?;

//             for id in existing_indexes.iter().rev() {
//                 let s = match app.search_service(*id) {
//                     Ok(s) => s,
//                     Err(e) => {
//                         log::warn!("unable to load search service {}: {}", id, e);
//                         continue;
//                     }
//                 };
//                 state.add_search(SearchApp::new(s))
//             }
//         }
//         LoadState::Ready(_) => panic!("don't try to load a ready state"),
//     };

//     Ok(())
// }

impl LoadedShell {
    pub fn load() -> anyhow::Result<LoadedShell> {
        log::info!("starting tokio runtime");
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("could not start tokio runtime")?;

        let lucile = rt
            .block_on(async { app::app::LucileBuilder::new()?.build().await })
            .context("could not load lucile app")?;

        Ok(LoadedShell {
            rt,
            lucile: std::sync::Arc::new(lucile),
        })
    }
}

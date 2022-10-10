use std::collections::HashMap;

use database::{Database, DatabaseError};
use lucile_core::identifiers::{ChapterId, MediaViewId};
use serde::{Deserialize, Serialize};

use crate::LucileAppError;

#[derive(Debug, Serialize, Deserialize)]
enum MediaViewDescriptor {
    Any,
    Latest,
    Exact(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct MediaViewPreferences {
    attempt_order: Vec<MediaViewDescriptor>,
}

impl Default for MediaViewPreferences {
    fn default() -> Self {
        Self {
            attempt_order: vec![MediaViewDescriptor::Latest, MediaViewDescriptor::Any],
        }
    }
}

async fn get_media_view_options(
    db: &Database,
    chapter_id: ChapterId,
) -> Result<Vec<(MediaViewId, String)>, LucileAppError> {
    Ok(db.get_media_view_options(chapter_id).await?)
}

struct MediaLookup<'a> {
    db: &'a Database,
    cache: HashMap<MediaViewId, bool>,
}

impl<'a> MediaLookup<'a> {
    fn new(db: &'a Database) -> MediaLookup<'a> {
        MediaLookup {
            db,
            cache: HashMap::new(),
        }
    }

    async fn check(&mut self, id: MediaViewId) -> Result<bool, DatabaseError> {
        self.db.


        Ok(true)
    }
}

async fn use_media_view_without_checking(
    db: &Database,
    chapter_id: ChapterId,
    description: &str,
) -> Result<MediaViewId, LucileAppError> {
    db.get_media_view_options(chapter_id)
        .await?
        .into_iter()
        .find(|o| o.1 == description)
        .map(|o| o.0)
        .ok_or(LucileAppError::MissingVideoSource)
}

async fn pick_media_view_with_preferences(
    db: &Database,
    chapter_id: ChapterId,
    preferences: &MediaViewPreferences,
) -> Result<Option<MediaViewId>, LucileAppError> {
    let options = db.get_media_view_options(chapter_id).await?;

    let mut media_checker = MediaLookup::new(db);

    for pref in &preferences.attempt_order {
        match pref {
            MediaViewDescriptor::Any => {
                for view in &options {
                    if media_checker.check(view.0).await? {
                        log::debug!("using media view: `{}`", view.1);
                        return Ok(Some(view.0));
                    }
                }
                return Ok(None);
            }
            MediaViewDescriptor::Latest => {
                if let Some(view) = options.first() {
                    if media_checker.check(view.0).await? {
                        log::debug!("using latest media view: `{}`", view.1);
                        return Ok(Some(view.0));
                    }
                }
            }
            MediaViewDescriptor::Exact(s) => {
                if let Some(view) = options.iter().find(|o| &o.1 == s) {
                    if media_checker.check(view.0).await? {
                        log::debug!("using media view `{}`", view.1);
                        return Ok(Some(view.0));
                    }
                }
            }
        }
    }
    Ok(None)
}

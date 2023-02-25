use lucile_core::{media_segment::MediaSegment, MediaHash};
use tokio::io::AsyncRead;

use crate::{app::LucileApp, LucileAppError};

/// Turn media hashes into contents we can consume
///
///
///

pub async fn get_reader_for_segment(
    app: &LucileApp,
    media_segment: &MediaSegment,
) -> Result<Box<dyn AsyncRead + Unpin + Send>, LucileAppError> {
    let mut content = get_reader_for_hash(app, media_segment.hash).await?;
    if let Some(key_data) = &media_segment.key {
        return Ok(crate::encryption::decryptor(key_data, &mut content).await?);
    }
    Ok(content)
}

pub async fn get_reader_for_hash(
    app: &LucileApp,
    hash: MediaHash,
) -> Result<Box<dyn AsyncRead + Unpin + Send>, LucileAppError> {
    let media = app
        .db
        .get_storage_by_hash(hash)
        .await?
        .ok_or_else(|| LucileAppError::MissingVideoSource)?;
    let f = tokio::fs::File::open(&media.path).await?;
    Ok(Box::new(f))
}

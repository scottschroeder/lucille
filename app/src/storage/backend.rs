use lucille_core::{media_segment::MediaSegment, MediaHash};
use tokio::io::AsyncRead;

pub(crate) use self::{db_storage::DbStorageBackend, local_media_root::MediaRootBackend};
use crate::{app::LucilleApp, LucilleAppError};

/// Turn media hashes into contents we can consume

pub(crate) struct MediaReader {
    rdr: Box<dyn AsyncRead + Unpin + Send>,
    src: BackendCacheControl,
}

pub(crate) enum BackendCacheControl {
    Local,
    Remote,
}

#[derive(Debug, Default)]
pub(crate) struct CascadingMediaBackend {
    inner: Vec<Box<dyn StorageBackend + Send + Sync>>,
}

impl CascadingMediaBackend {
    pub(crate) fn push_back(&mut self, backend: impl StorageBackend + Send + Sync + 'static) {
        self.inner.push(Box::new(backend))
    }
    pub(crate) async fn get_media_by_hash(
        &self,
        hash: MediaHash,
    ) -> Result<MediaReader, LucilleAppError> {
        for backend in &self.inner {
            log::trace!("looking up media {} from {}", hash, backend.name());
            if let Some(rdr) = backend.get_media_by_hash(hash).await? {
                return Ok(MediaReader {
                    rdr,
                    src: backend.cache_control(),
                });
            }
        }
        Err(LucilleAppError::MissingVideoSource)
    }
}

#[async_trait::async_trait]
pub(crate) trait StorageBackend: std::fmt::Debug {
    async fn get_media_by_hash(
        &self,
        hash: MediaHash,
    ) -> Result<Option<Box<dyn AsyncRead + Unpin + Send>>, LucilleAppError>;
    fn cache_control(&self) -> BackendCacheControl;
    fn name(&self) -> &'static str;
}

fn wrap_io_notfound<T>(e: std::io::Error) -> Result<Option<T>, LucilleAppError> {
    match e.kind() {
        std::io::ErrorKind::NotFound => Ok(None),
        _ => Err(e.into()),
    }
}

mod db_storage {
    use database::Database;
    use lucille_core::MediaHash;
    use tokio::io::AsyncRead;

    use super::{wrap_io_notfound, BackendCacheControl, StorageBackend};
    use crate::LucilleAppError;

    pub(crate) struct DbStorageBackend {
        db: Database,
    }

    impl DbStorageBackend {
        pub(crate) fn new(db: Database) -> Self {
            Self { db }
        }
    }

    impl std::fmt::Debug for DbStorageBackend {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("DbStorageBackend").finish()
        }
    }

    #[async_trait::async_trait]
    impl StorageBackend for DbStorageBackend {
        async fn get_media_by_hash(
            &self,
            hash: MediaHash,
        ) -> Result<Option<Box<dyn AsyncRead + Unpin + Send>>, LucilleAppError> {
            let media = self
                .db
                .get_storage_by_hash(hash)
                .await?
                .ok_or_else(|| LucilleAppError::MissingVideoSource)?;
            match tokio::fs::File::open(&media.path).await {
                Ok(f) => Ok(Some(Box::new(f))),
                Err(e) => wrap_io_notfound(e),
            }
        }
        fn cache_control(&self) -> BackendCacheControl {
            BackendCacheControl::Local
        }

        fn name(&self) -> &'static str {
            "DbStorageBackend"
        }
    }
}

mod local_media_root {
    use lucille_core::MediaHash;
    use tokio::io::AsyncRead;

    use super::{wrap_io_notfound, BackendCacheControl, StorageBackend};
    use crate::{hashfs::HashFS, LucilleAppError};

    #[derive(Debug)]
    pub(crate) struct MediaRootBackend {
        inner: HashFS,
    }

    impl MediaRootBackend {
        pub(crate) fn new(inner: HashFS) -> Self {
            Self { inner }
        }
    }

    #[async_trait::async_trait]
    impl StorageBackend for MediaRootBackend {
        async fn get_media_by_hash(
            &self,
            hash: MediaHash,
        ) -> Result<Option<Box<dyn AsyncRead + Unpin + Send>>, LucilleAppError> {
            let path = self.inner.get_file_path(hash);
            match tokio::fs::File::open(&path).await {
                Ok(f) => Ok(Some(Box::new(f))),
                Err(e) => wrap_io_notfound(e),
            }
        }
        fn cache_control(&self) -> BackendCacheControl {
            BackendCacheControl::Local
        }
        fn name(&self) -> &'static str {
            "MediaRoot"
        }
    }
}

#[cfg(feature = "aws-sdk")]
mod s3_media_root {
    use lucille_core::MediaHash;
    use tokio::io::AsyncRead;

    use super::{BackendCacheControl, StorageBackend};
    use crate::LucilleAppError;

    #[derive(Debug)]
    pub(crate) struct S3MediaBackend {}

    #[async_trait::async_trait]
    impl StorageBackend for S3MediaBackend {
        async fn get_media_by_hash(
            &self,
            hash: MediaHash,
        ) -> Result<Option<Box<dyn AsyncRead + Unpin + Send>>, LucilleAppError> {
            return Err(LucilleAppError::MissingVideoSource);
        }
        fn cache_control(&self) -> BackendCacheControl {
            BackendCacheControl::Remote
        }
        fn name(&self) -> &'static str {
            "S3"
        }
    }
}

pub async fn get_reader_for_segment(
    app: &LucilleApp,
    media_segment: &MediaSegment,
) -> Result<Box<dyn AsyncRead + Unpin + Send>, LucilleAppError> {
    let mut content = app.storage.get_media_by_hash(media_segment.hash).await?;
    // let mut content = get_reader_for_hash(app, media_segment.hash).await?;
    if let Some(key_data) = &media_segment.key {
        return Ok(crate::encryption::decryptor(key_data, &mut content.rdr).await?);
    }
    Ok(content.rdr)
}

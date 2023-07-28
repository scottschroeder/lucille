use database::Database;
use lucille_core::{
    metadata::{MediaHash, MediaMetadata},
    Corpus,
};

mod extract;
mod insert;
mod metadata;
mod scan;

#[derive(Debug, thiserror::Error)]
#[deprecated(note = "use anyhow")]
pub enum ScanError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub enum ScannedSubtitles {
    NotFound,
    Error(subrip::Error),
    Subtitles(Vec<subrip::Subtitle>),
}

impl PartialEq for ScannedSubtitles {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Error(l0), Self::Error(r0)) => match (l0, r0) {
                (
                    subrip::Error::ParseError(lsubs, lerr),
                    subrip::Error::ParseError(rsubs, rerr),
                ) => lsubs == rsubs && lerr == rerr,
                _ => false,
            },
            (Self::Subtitles(l0), Self::Subtitles(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl std::fmt::Debug for ScannedSubtitles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "NotFound"),
            Self::Error(arg0) => f.debug_tuple("Error").field(arg0).finish(),
            Self::Subtitles(arg0) => f.debug_tuple("Subtitles").field(&arg0.len()).finish(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ScannedData {
    pub path: std::path::PathBuf,
    pub subs: ScannedSubtitles,
    pub hash: MediaHash,
}

#[derive(Debug, PartialEq)]
pub struct ScannedMedia {
    pub path: std::path::PathBuf,
    pub subs: ScannedSubtitles,
    pub hash: MediaHash,
    pub metadata: MediaMetadata,
}

impl LucilleApp {
    pub fn media_scanner(&self, trust_hashes: bool) -> MediaProcessor {
        MediaProcessor {
            db: self.db.clone(),
            trust_hashes,
        }
    }
}

#[derive(Clone)]
pub struct MediaProcessor {
    pub db: Database,
    pub trust_hashes: bool,
}

pub use insert::add_content_to_corpus;
pub use scan::scan_media_paths;

use crate::{app::LucilleApp, LucilleAppError};

impl MediaProcessor {
    pub async fn ingest<P: AsRef<std::path::Path>>(
        &self,
        root: P,
        corpus: Option<&Corpus>,
    ) -> Result<(), LucilleAppError> {
        let content = self.scan_and_process(root).await?;

        add_content_to_corpus(&self.db, corpus, content).await
    }
    async fn scan_and_process<P: AsRef<std::path::Path>>(
        &self,
        root: P,
    ) -> std::io::Result<Vec<ScannedMedia>> {
        let media_paths = scan::scan_media_paths(root)?;
        Ok(self.process_all_media(&media_paths).await)
    }
    pub async fn process_all_media(&self, paths: &[std::path::PathBuf]) -> Vec<ScannedMedia> {
        let mut set = tokio::task::JoinSet::new();
        let mut res = vec![];
        for p in paths {
            let media_path = p.clone();
            let p = self.clone();
            set.spawn(async move {
                let r = p.process_single_media3(&media_path).await;
                (media_path, r)
            });
        }

        while let Some(join_res) = set.join_next().await {
            match join_res {
                Ok((_, Ok(m))) => res.push(m),
                Ok((p, Err(e))) => log::warn!("unable to use {:?}: {}", p, e),
                Err(e) => log::error!("unable to join processing task: {}", e),
            }
        }

        res
    }

    async fn process_single_media3(
        &self,
        media_path: &std::path::Path,
    ) -> anyhow::Result<ScannedMedia> {
        extract::read_media_from_path(&self.db, media_path, self.trust_hashes)
            .await
            .map(|data| data.extract_metadata())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, io::Write, path::PathBuf};

    use lucille_core::test_util::generate_subtitle;

    use super::*;
    use crate::app::tests::lucille_test_app;

    struct TestMedia {
        root: tempfile::TempDir,
        media: HashMap<PathBuf, ScannedMedia>,
    }

    fn create_simple_show_structure() -> TestMedia {
        let root = tempfile::tempdir().expect("could not create tmpdir");
        let mut media = HashMap::new();
        let show_name = "showname";
        let show_path = root.path().join(show_name);
        std::fs::create_dir(&show_path).expect("could not create show path");
        for s in 0..3 {
            let season_path = show_path.join(format!("Season {}", s));
            std::fs::create_dir(&season_path).expect("could not create show path");
            for e in 0..12 {
                let ep_title = format!("ep_name_{}_{}", s, e);
                let ep_fname = format!("{}.S{}E{}.{}.ext", show_name, s, e, ep_title);
                let ep_base = season_path.join(ep_fname);
                let video_path = ep_base.with_extension("mkv");

                let mut f_video = std::fs::File::create(&video_path).expect("create video file");
                f_video
                    .write_all(ep_title.as_bytes())
                    .expect("unable to write video file");

                let hash = MediaHash::from_bytes(ep_title.as_bytes());

                let srt_path = ep_base.with_extension("srt");
                let f_srt = std::fs::File::create(srt_path).expect("create srt file");
                let mut w = std::io::BufWriter::new(f_srt);
                let srt_data = generate_subtitle(&[
                    "show subs",
                    &format!("season {}", s),
                    &format!("episode {}", e),
                ]);
                for sub in &srt_data {
                    write!(w, "{}", sub).expect("unable to write sub line")
                }
                let expected = ScannedMedia {
                    path: video_path.clone(),
                    subs: ScannedSubtitles::Subtitles(srt_data),
                    hash,
                    metadata: MediaMetadata::Episode(lucille_core::metadata::EpisodeMetadata {
                        season: s,
                        episode: e,
                        title: ep_title,
                    }),
                };
                media.insert(video_path, expected);
            }
        }

        TestMedia { root, media }
    }

    fn assert_scanned_media(actual: &ScannedMedia, expected: &ScannedMedia) {
        if actual.subs != expected.subs {
            if let (ScannedSubtitles::Subtitles(ms), ScannedSubtitles::Subtitles(es)) =
                (&actual.subs, &expected.subs)
            {
                for (l, r) in ms.iter().zip(es.iter()) {
                    assert_eq!(l, r)
                }
            }
        }

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn scan_and_extract_media() {
        let test_app = lucille_test_app().await;
        let test_media = create_simple_show_structure();

        let media = test_app
            .app
            .media_scanner(false)
            .scan_and_process(test_media.root.path())
            .await
            .expect("scan and process");

        for m in &media {
            let expected = &test_media.media[&m.path];
            assert_scanned_media(m, expected)
        }
    }

    #[tokio::test]
    async fn scan_and_extract_with_wrong_hash_in_db() {
        let test_app = lucille_test_app().await;
        let test_media = create_simple_show_structure();

        let (path, _) = test_media.media.iter().next().expect("no media");
        let garbage_hash = MediaHash::from_bytes(b"total garbage");
        test_app
            .app
            .db
            .add_storage(garbage_hash, path)
            .await
            .unwrap();

        let media = test_app
            .app
            .media_scanner(false)
            .scan_and_process(test_media.root.path())
            .await
            .expect("scan and process");

        for m in &media {
            if &m.path != path {
                continue;
            }
            let expected = &test_media.media[&m.path];
            assert_scanned_media(m, expected)
        }
    }

    #[tokio::test]
    async fn scan_and_extract_with_wrong_hash_in_db_with_trust() {
        let test_app = lucille_test_app().await;
        let test_media = create_simple_show_structure();

        let (path, _) = test_media.media.iter().next().expect("no media");
        let garbage_hash = MediaHash::from_bytes(b"total garbage");
        test_app
            .app
            .db
            .add_storage(garbage_hash, path)
            .await
            .unwrap();

        let media = test_app
            .app
            .media_scanner(true)
            .scan_and_process(test_media.root.path())
            .await
            .expect("scan and process");

        for m in &media {
            if &m.path != path {
                continue;
            }
            assert_eq!(m.hash, garbage_hash)
        }
    }
}

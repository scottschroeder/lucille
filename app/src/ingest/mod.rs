use database::Database;
use lucile_core::metadata::{MediaHash, MediaMetadata};
use rayon::prelude::*;

mod extract;
mod insert;
mod metadata;
mod scan;

#[derive(Debug, thiserror::Error)]
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

#[derive(Clone)]
pub struct MediaProcessor {
    pub db: Database,
    pub trust_hashes: bool,
}

pub use insert::add_content_to_corpus;
pub use scan::scan_media_paths;

impl MediaProcessor {
    pub async fn process_all_media(&self, paths: &[std::path::PathBuf]) -> Vec<ScannedMedia> {
        // let mut set = tokio::task::JoinSet::new();
        let mut res = vec![];
        for p in paths {
            match self.process_single_media3(p).await {
                Ok(s) => res.push(s),
                Err(e) => log::warn!("unable to use {:?}: {}", p, e),
            }
            // let media_path = p.clone();
            // let p = processor.clone();
            // set.spawn(async move {
            //     p.process_single_media3(&media_path)
            // });
        }
        res
    }

    // /// batch process a list of media paths
    // pub async fn process_media_in_parallel(&self, paths: &[std::path::PathBuf]) -> Vec<ScannedMedia> {
    //     paths
    //         .into_par_iter()
    //         .map(|p| (p, self.process_single_media3(p.as_path())))
    //         .filter_map(|(p, r)| match r {
    //             Ok(m) => Some((p.to_owned(), m)),
    //             Err(e) => {
    //                 log::warn!("unable to use {:?}: {}", p, e);
    //                 None
    //             }
    //         })
    //         .map(|(_p, r)| r)
    //         .collect()
    // }
    async fn process_single_media3(
        &self,
        media_path: &std::path::Path,
    ) -> Result<ScannedMedia, ScanError> {
        extract::read_media_from_path(&self.db, media_path, self.trust_hashes)
            .await
            .map(|data| data.extract_metadata())
    }
}

/*
 * I'm trying to take `app/src/scan.rs` and break it apart.
 *
 * ingest::scan should trawl the FS for media paths
 * ingest::extract should do anything that immediately requires the FS
 *  - in practice, that is getting the media hash, and extracting subs
 * ingest::<what> should do the logical application update to the db and
 *  we should test the bejesus out of it
 */

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, io::Write, path::PathBuf};

    use lucile_core::test_util::generate_subtitle;

    use super::*;
    use crate::app::tests::lucile_test_app;

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
                    metadata: MediaMetadata::Episode(lucile_core::metadata::EpisodeMetadata {
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

    #[tokio::test]
    async fn scan_and_extract_media() {
        let test_app = lucile_test_app().await;
        let test_media = create_simple_show_structure();

        let media_paths = scan_media_paths(test_media.root.path()).expect("scan");
        let processor = MediaProcessor {
            db: test_app.app.db.clone(),
            trust_hashes: false,
        };
        let media = processor.process_all_media(&media_paths).await;

        for m in &media {
            let expected = &test_media.media[&m.path];

            if m.subs != expected.subs {
                if let (ScannedSubtitles::Subtitles(ms), ScannedSubtitles::Subtitles(es)) =
                    (&m.subs, &expected.subs)
                {
                    for (l, r) in ms.iter().zip(es.iter()) {
                        assert_eq!(l, r)
                    }
                }
            }

            assert_eq!(m, expected);
        }
    }
}

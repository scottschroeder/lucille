use std::path;

use lucille_core::metadata::{EpisodeMetadata, MediaMetadata};

use super::{ScannedData, ScannedMedia};

impl ScannedData {
    pub(crate) fn extract_metadata(self) -> ScannedMedia {
        let metadata = extract_metadata_from_path(self.path.as_path());
        ScannedMedia {
            path: self.path,
            subs: self.subs,
            hash: self.hash,
            metadata,
        }
    }
}

fn extract_metadata_from_path(fname: &path::Path) -> MediaMetadata {
    let file_name = fname.file_name().expect("path did not contain filename");
    let title = file_name.to_string_lossy().to_string();
    extract_metadata(&title)
}

/// Get rich-ish metadata from a media's title
fn extract_metadata(title: &str) -> MediaMetadata {
    match torrent_name_parser::Metadata::from(title) {
        Ok(m) => {
            if let Some((s, e)) = m.season().zip(m.episode()) {
                MediaMetadata::Episode(EpisodeMetadata {
                    season: s as u32,
                    episode: e as u32,
                    title: hacky_extract_episode_name(title),
                })
            } else {
                MediaMetadata::Unknown(m.title().to_string())
            }
        }
        Err(e) => {
            log::warn!("could not parse metadata from `{:?}`: {}", title, e);
            MediaMetadata::Unknown(title.to_string())
        }
    }
}

// TODO: this only happens to work because of my data
fn hacky_extract_episode_name(filename: &str) -> String {
    let segments = filename.split('.').collect::<Vec<_>>();
    let r = segments[2..segments.len() - 1].join(".");
    if r.is_empty() {
        filename.to_string()
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_packed_show_path() {
        let p = path::Path::new("./path/dir/Show Name.S03E12.Episode Title.mkv");
        let metadata = extract_metadata_from_path(p);
        assert_eq!(
            metadata,
            MediaMetadata::Episode(EpisodeMetadata {
                season: 3,
                episode: 12,
                title: "Episode Title".to_owned(),
            })
        )
    }
}

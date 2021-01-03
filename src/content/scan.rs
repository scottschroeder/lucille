use super::{Content, Episode, FileSystemContent, VideoFile};
use crate::srt::Subtitle;
use anyhow::Result;
use std::{fmt, io::Read, path};

const MEDIA_FILES: &[&str] = &["mkv"];

pub struct EpisodeFiles {
    video: String,
    title: String,
    subtitles: Vec<Subtitle>,
}

impl fmt::Debug for EpisodeFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EpisodeFiles")
            .field("video", &self.video)
            .field("title", &self.title)
            .field("subtitles", &format_args!("[{}]", self.subtitles.len()))
            .finish()
    }
}

fn is_media(p: &path::Path) -> bool {
    let oext = p.extension();
    oext.and_then(|ext| ext.to_str())
        .map(|ext| MEDIA_FILES.contains(&ext))
        .unwrap_or(false)
}

pub fn scan_filesystem<P: AsRef<path::Path>>(root: P) -> Result<(Content, FileSystemContent)> {
    let root = root.as_ref();
    let mut episodes = Vec::new();
    let mut videos = Vec::new();
    for dir in walkdir::WalkDir::new(root)
        .into_iter()
        .filter(|de| de.as_ref().map(|de| is_media(de.path())).unwrap_or(true))
    {
        let dir = dir?;
        let media_path = dir.path();
        let srt_path = media_path.with_extension("srt");

        let subs = match read_file(srt_path.as_path()).and_then(|s| crate::srt::parse(s.as_str())) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("unable to load subtitles for {:?}: {}", srt_path, e);
                continue;
            }
        };
        let fname = media_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("video file path was not utf8"))?
            .to_owned();
        episodes.push(Episode {
            title: title(media_path)?,
            subtitles: subs,
        });
        videos.push(VideoFile(fname));
    }

    Ok((Content { episodes }, FileSystemContent { videos }))
}

fn title(p: &path::Path) -> Result<String> {
    let fname = p
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| anyhow::anyhow!("media path was not utf8"))?;
    let title = if let Some(idx) = fname.rfind('.') {
        &fname[..idx]
    } else {
        fname
    };
    Ok(title.to_string())
}

fn read_file<P: AsRef<path::Path>>(tpath: P) -> Result<String> {
    let tpath = tpath.as_ref();
    let mut f = std::fs::File::open(tpath)?;
    let mut v = Vec::new();
    f.read_to_end(&mut v)?;

    Ok(match String::from_utf8(v) {
        Ok(s) => s,
        Err(e) => {
            let v = e.into_bytes();
            // SRT files are WINDOWS_1252 by default, but there is no requirement, so who knows
            let (text, encoding, replacements) = encoding_rs::WINDOWS_1252.decode(v.as_slice());
            if replacements {
                log::warn!(
                    "could not decode {:?} accurately with {}",
                    tpath,
                    encoding.name()
                );
            }
            text.to_string()
        }
    })
}

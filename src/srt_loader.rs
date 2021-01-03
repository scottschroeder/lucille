use crate::{content::Episode, srt::Subtitle};
use anyhow::Result;
use path::PathBuf;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, io::Read, path};

const CONTENT_DIR_KEY: &str = "CONTENT_DIR";

fn content_dir() -> Result<path::PathBuf> {
    let source = std::env::var_os(CONTENT_DIR_KEY)
        .ok_or_else(|| anyhow::anyhow!("must set {:?} to path of content"))?;
    Ok(PathBuf::from(source))
}

pub fn generate_multi_window(
    size: usize,
    max_window: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (0..max_window).flat_map(move |window| (0..(size - window)).map(move |s| (s, s + window + 1)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexableEpisode {
    pub title: String,
    pub script: String,
    pub subs: Vec<Subtitle>,
    pub index: Vec<usize>,
}

pub struct Clip<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
}

impl From<Episode> for IndexableEpisode {
    fn from(e: Episode) -> Self {
        let Episode {
            title,
            subtitles: subs,
        } = e;
        let mut script = String::new();
        let mut index = vec![0];

        for sub in &subs {
            for line in sub.text.lines() {
                let text = line.trim().trim_start_matches('-').trim();
                script.push_str(" ");
                script.push_str(text);
            }
            index.push(script.len())
        }

        IndexableEpisode {
            title,
            script,
            subs,
            index,
        }
    }
}

pub struct CleanSubs<'a>(pub &'a [Subtitle]);

impl<'a> fmt::Display for CleanSubs<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for sub in self.0 {
            for line in sub.text.lines() {
                let text = line.trim().trim_start_matches('-').trim();
                f.write_str(" ")?;
                f.write_str(text)?;
            }
        }
        Ok(())
    }
}

impl IndexableEpisode {
    fn from_subs(title: String, subs: Vec<Subtitle>) -> IndexableEpisode {
        let mut script = String::new();
        let mut index = vec![0];

        for sub in &subs {
            for line in sub.text.lines() {
                let text = line.trim().trim_start_matches('-').trim();
                script.push_str(" ");
                script.push_str(text);
            }
            index.push(script.len())
        }

        IndexableEpisode {
            title,
            script,
            subs,
            index,
        }
    }
    pub fn extract_window(&self, start: usize, end: usize) -> &str {
        let start_byte = self.index[start];
        let end_byte = if end < self.index.len() {
            self.index[end]
        } else {
            self.script.len()
        };
        &self.script[start_byte..end_byte]
    }

    pub fn slices<'a>(&'a self, max_window: usize) -> impl Iterator<Item = Clip<'a>> + 'a {
        generate_multi_window(self.subs.len(), max_window).map(move |(start, end)| Clip {
            title: self.title.as_str(),
            text: self.extract_window(start, end),
            start,
            end,
        })
    }
}

fn is_srt(p: &path::Path) -> bool {
    let oext = p.extension();
    oext.map(|ext| ext == "srt").unwrap_or(false)
}

fn rough_title(p: &path::Path) -> String {
    let fname = p
        .file_name()
        .map(|oss| oss.to_string_lossy())
        .unwrap_or("unknown".into());
    // let fname = fname.split('.').next().unwrap_or(fname.as_ref());
    fname.to_string()
}

fn list_subs<P: AsRef<path::Path>>(root: P) -> Result<HashMap<String, Vec<Subtitle>>> {
    let root = root.as_ref();
    let mut subs = HashMap::new();
    for dir in walkdir::WalkDir::new(root)
        .into_iter()
        .filter(|de| de.as_ref().map(|de| is_srt(de.path())).unwrap_or(true))
    {
        let dir = dir?;
        let name = rough_title(dir.path());
        log::trace!("open subtitles {:?}", dir.path());

        match read_file(dir.path()).and_then(|s| crate::srt::parse(s.as_str())) {
            Ok(s) => {
                subs.insert(name, s);
            }
            Err(e) => log::error!("{:?}: {}", dir.path(), e),
        }
    }

    Ok(subs)
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

pub fn parse_adsubs() -> Result<Vec<IndexableEpisode>> {
    Ok(list_subs(content_dir()?)?
        .into_iter()
        .map(|(t, s)| IndexableEpisode::from_subs(t, s))
        .collect::<Vec<_>>())
}

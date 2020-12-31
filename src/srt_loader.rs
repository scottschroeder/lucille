use crate::srt::Subtitle;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Read, path};

const ADSUBS: &str = "/home/scott/Dropbox/Development/ArrestedDevelopmentSubs";

pub fn generate_multi_window(
    size: usize,
    max_window: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (0..max_window).flat_map(move |window| (0..(size - window)).map(move |s| (s, s + window + 1)))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Episode {
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

impl Episode {
    fn from_subs(title: String, subs: Vec<Subtitle>) -> Episode {
        let mut script = String::new();
        let mut index = Vec::new();

        for sub in &subs {
            for line in sub.text.lines() {
                let text = line.trim().trim_start_matches('-').trim();
                script.push_str(" ");
                script.push_str(text);
            }
            index.push(script.len())
        }

        Episode {
            title,
            script,
            subs,
            index,
        }
    }
    fn extract_window(&self, start: usize, end: usize) -> &str {
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
    let fname = fname.split('.').next().unwrap_or(fname.as_ref());
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

// TODO actually parse SRT
fn srt_to_script(s: &str) -> Result<String> {
    // log::debug!("{:#?}", vsub);
    let mut r = String::new();
    let mut reset = 2;
    for line in s.lines() {
        let text = line.trim().trim_start_matches('-').trim();
        if text.is_empty() {
            reset = 2;
            continue;
        }
        if reset > 0 {
            reset -= 1;
            continue;
        }
        r.push_str(" ");
        r.push_str(text);
    }
    Ok(r)
}

pub fn script_splitter(s: &str) -> Vec<String> {
    s.split(". ")
        .flat_map(|s| s.split('!'))
        .flat_map(|s| s.split("<i>"))
        .flat_map(|s| s.split("</i>"))
        .flat_map(|s| s.split('?'))
        .flat_map(|s| s.split('{'))
        .flat_map(|s| s.split('}'))
        .flat_map(|s| s.split('['))
        .flat_map(|s| s.split(']'))
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .map(|s| s.to_owned())
        .collect()
}

pub fn parse_adsubs() -> Result<Vec<Episode>> {
    Ok(list_subs(ADSUBS)?
        .into_iter()
        .map(|(t, s)| Episode::from_subs(t, s))
        .collect::<Vec<_>>())
}

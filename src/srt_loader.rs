use anyhow::{Context, Result};
use std::collections::HashMap;
use std::{io::Read, path};

const ADSUBS: &str = "/home/scott/Dropbox/Development/ArrestedDevelopmentSubs";

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

fn list_subs<P: AsRef<path::Path>>(root: P) -> Result<HashMap<String, String>> {
    let root = root.as_ref();
    let mut subs = HashMap::new();
    for dir in walkdir::WalkDir::new(root)
        .into_iter()
        .filter(|de| de.as_ref().map(|de| is_srt(de.path())).unwrap_or(true))
    {
        let dir = dir?;
        let name = rough_title(dir.path());
        log::trace!("open subtitles {:?}", dir.path());

        match read_file(dir.path()).and_then(|s| srt_to_script(s.as_ref())) {
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
                log::warn!("could not decode {:?} accurately with {}", tpath, encoding.name());
            }
            text.to_string()
        }
    })
}

// TODO actually parse SRT
fn srt_to_script(s: &str) -> Result<String> {
    let vsub = crate::srt::parse(s);
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

pub fn parse_adsubs() -> Result<HashMap<String, String>> {
    Ok(list_subs(ADSUBS)?)
}

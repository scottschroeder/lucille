use crate::{content::ContentData, srt::Subtitles};
use serde::{Deserialize, Serialize};

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
    pub subs: Subtitles,
    pub index: Vec<usize>,
}

pub struct Clip<'a> {
    pub title: &'a str,
    pub text: &'a str,
    pub start: usize,
    pub end: usize,
}

impl From<ContentData> for IndexableEpisode {
    fn from(c: ContentData) -> Self {
        let ContentData {
            subtitle: subs,
            media_hash: _,
            metadata,
        } = c;
        let mut script = String::new();
        let mut index = vec![0];

        for sub in &subs.inner {
            for line in sub.text.lines() {
                let text = line.trim().trim_start_matches('-').trim();
                script.push_str(" ");
                script.push_str(text);
            }
            index.push(script.len())
        }

        IndexableEpisode {
            title: metadata.title(),
            script,
            subs,
            index,
        }
    }
}

impl IndexableEpisode {
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
        generate_multi_window(self.subs.inner.len(), max_window).map(move |(start, end)| Clip {
            title: self.title.as_str(),
            text: self.extract_window(start, end),
            start,
            end,
        })
    }
}
use lucile_core::{ContentData, Subtitle};

pub fn generate_multi_window(
    size: usize,
    max_window: usize,
) -> impl Iterator<Item = (usize, usize)> {
    (0..max_window).flat_map(move |window| (0..(size - window)).map(move |s| (s, s + window + 1)))
}

pub struct IndexableEpisode {
    pub title: String,
    pub srt_id: i64,
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

impl From<ContentData> for IndexableEpisode {
    fn from(c: ContentData) -> Self {
        let ContentData {
            subtitle: subs,
            metadata,
            hash: _,
        } = c;
        let mut script = String::new();
        let mut index = vec![0];

        for sub in &subs.subs {
            for line in sub.text.lines() {
                let text = line.trim().trim_start_matches('-').trim();
                script.push(' ');
                script.push_str(text);
            }
            index.push(script.len())
        }

        IndexableEpisode {
            title: metadata.title(),
            srt_id: subs.id,
            script,
            subs: subs.subs,
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

    pub fn slices(&self, max_window: usize) -> impl Iterator<Item = Clip<'_>> + '_ {
        generate_multi_window(self.subs.len(), max_window).map(move |(start, end)| Clip {
            title: self.title.as_str(),
            text: self.extract_window(start, end),
            start,
            end,
        })
    }
}

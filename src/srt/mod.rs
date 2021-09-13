use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};

mod parser;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtitles {
    pub inner: Vec<Subtitle>,
}

impl Subtitles {
    pub fn parse(input: &str) -> Result<Subtitles> {
        let subs = crate::srt::parse(input)?;
        Ok(Subtitles::new(subs))
    }

    pub fn new(subs: Vec<Subtitle>) -> Subtitles {
        Subtitles { inner: subs }
    }
}

impl fmt::Debug for Subtitles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Subtitles({})", self.inner.len())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtitle {
    pub idx: u32,
    pub start: Duration,
    pub end: Duration,
    pub text: String,
}

fn fmt_duration_srt(d: Duration, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let millis = d.as_millis();
    let secs = millis / 1000;
    let millis = millis % 1000;
    let minutes = secs / 60;
    let secs = secs % 60;
    let hours = minutes / 60;
    let minutes = minutes % 60;
    write!(f, "{}:{}:{},{}", hours, minutes, secs, millis)
}

impl fmt::Display for Subtitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.idx)?;
        fmt_duration_srt(self.start, f)?;
        write!(f, " --> ")?;
        fmt_duration_srt(self.end, f)?;
        writeln!(f, "")?;
        writeln!(f, "{}", self.text)?;
        writeln!(f, "")
    }
}

pub fn offset_subs(delay_start: Option<Duration>, subs: &[Subtitle]) -> Vec<Subtitle> {
    if subs.is_empty() {
        return vec![];
    }
    let base = subs[0].start - delay_start.unwrap_or_else(|| Duration::from_micros(0));
    subs.iter()
        .enumerate()
        .map(|(idx, s)| Subtitle {
            idx: idx as u32,
            start: s.start - base,
            end: s.end - base,
            text: s.text.clone(),
        })
        .collect()
}
pub use parser::parse;

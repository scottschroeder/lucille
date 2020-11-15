use nom;
use std::time::Duration;

mod parser;

#[derive(Debug, Clone, PartialEq)]
pub struct Subtitle {
    idx: u32,
    start: Duration,
    end: Duration,
    text: String,
}

pub use parser::parse;


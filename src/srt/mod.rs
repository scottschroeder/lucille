use serde::{Deserialize, Serialize};
use std::time::Duration;

mod parser;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subtitle {
    pub idx: u32,
    pub start: Duration,
    pub end: Duration,
    pub text: String,
}

pub use parser::parse;

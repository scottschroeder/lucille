use anyhow::Result;
use uuid::Uuid;

pub struct ClipIdentifier {
    pub index: Uuid,
    pub episode: usize,
    pub start: usize,
    pub end: usize,
}

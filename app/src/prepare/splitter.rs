use std::time::Duration;

pub struct MediaSplitter<'a> {
    source: &'a std::path::Path,
    target_duration: Duration,
}

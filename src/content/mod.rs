use std::borrow::Cow;

pub mod scan;

pub trait VideoSource {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str>;
    fn ffmpeg_type(&self) -> Option<String> {
        None
    }
}

struct FileSystemContent {
    videos: Vec<VideoFile>,
}

struct VideoFile(String);

impl VideoSource for VideoFile {
    fn ffmpeg_src<'a>(&'a self) -> Cow<'a, str> {
        self.0.as_str().into()
    }
}

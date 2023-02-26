use std::path;
const MEDIA_FILES: &[&str] = &["mkv"];

/// Get the list of paths
pub fn scan_media_paths<P: AsRef<path::Path>>(root: P) -> std::io::Result<Vec<path::PathBuf>> {
    let root = root.as_ref();
    let mut content = Vec::new();
    for dir in walkdir::WalkDir::new(root)
        .into_iter()
        .filter(|de| de.as_ref().map(|de| is_media(de.path())).unwrap_or(true))
    {
        let dir = dir?;
        log::trace!("scanned: {:?}", dir.path());
        content.push(dir.path().to_owned());
    }

    Ok(content)
}
/// is a path media we care about?
fn is_media(p: &path::Path) -> bool {
    let oext = p.extension();
    oext.and_then(|ext| ext.to_str())
        .map(|ext| MEDIA_FILES.contains(&ext))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_srt_is_not_media() {
        assert!(!is_media(path::Path::new("my/file.srt")))
    }

    #[test]
    fn check_mkv_is_media() {
        assert!(is_media(path::Path::new("my/file.mkv")))
    }

    #[test]
    fn check_no_extension_is_not_media() {
        assert!(!is_media(path::Path::new("my/file")))
    }
}

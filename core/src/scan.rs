use std::{collections::HashMap, path};

const MEDIA_FILES: &[&str] = &["mkv"];

/// is a path media we care about?
fn is_media(p: &path::Path) -> bool {
    let oext = p.extension();
    oext.and_then(|ext| ext.to_str())
        .map(|ext| MEDIA_FILES.contains(&ext))
        .unwrap_or(false)
}

/// Get the list of paths
fn scan_media_paths<P: AsRef<path::Path>>(root: P) -> std::io::Result<Vec<path::PathBuf>> {
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

/// Get all content under a directory
pub fn scan_content<P: AsRef<path::Path>>(root: P) -> std::io::Result<ContentScanResults> {
    let contents = scan_media_paths(root)?;
    let media = process_media_in_parallel(contents.as_slice());
    Ok(content_metadata(media))
}

/// find/extract subtitles for a given piece of media
fn extract_subtitles(media_path: &path::Path) -> Result<Subtitles> {
    let srt_path = media_path.with_extension("srt");
    path_to_string(srt_path.as_path()).and_then(|s| Subtitles::parse(s.as_str()))
}
///
/// Get the sha2 hash for a media path
fn hash_video(p: &path::Path) -> Result<MediaHash> {
    let mut file =
        std::fs::File::open(p).with_context(|| format!("could not open file: {:?}", p))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .with_context(|| format!("could not hash file: {:?}", p))?;

    Ok(MediaHash::new(Sha2Hash::from(hasher.finalize())))
}

/// get details about a single piece of media
fn process_single_media(media_path: &path::Path) -> Result<ContentFileDetails> {
    let subtitles = extract_subtitles(media_path)?;
    let media_hash = hash_video(media_path)?;
    let title = title(media_path)?;
    let episode = ContentFileDetails {
        title,
        subtitles,
        media_hash,
    };
    log::trace!("{:?}", episode);
    Ok(episode)
}

/// batch process a list of media paths
fn process_media_in_parallel(
    paths: &[path::PathBuf],
) -> Vec<(path::PathBuf, ContentFileDetails)> {
    paths
        .into_par_iter()
        .map(|p| (p, process_single_media(p.as_path())))
        .filter_map(|(p, r)| match r {
            Ok(m) => Some((p.to_owned(), m)),
            Err(e) => {
                log::warn!("unable to use {:?}: {}", p, e);
                None
            }
        })
        .collect()
}

/// get the title for a media path
/// TODO: this is a really primitive implementation
fn title(p: &path::Path) -> Result<String> {
    let fname = p
        .file_name()
        .and_then(|os| os.to_str())
        .ok_or_else(|| anyhow::anyhow!("media path was not utf8"))?;
    return Ok(fname.to_string());
}
/// Get rich-ish metadata from a media's title
fn extract_metadata(title: &str) -> (MediaMetadata, Option<String>) {
    let mut name_guess = None;
    let metadata = match torrent_name_parser::Metadata::from(title) {
        Ok(m) => {
            name_guess = Some(m.title().to_string());
            if let Some((s, e)) = m.season().zip(m.episode()) {
                MediaMetadata::Episode(EpisodeMetadata {
                    season: s as u32,
                    episode: e as u32,
                    title: title.to_string(),
                })
            } else {
                MediaMetadata::Unknown(m.title().to_string())
            }
        }
        Err(e) => {
            log::warn!("could not parse metadata from `{:?}`: {}", title, e);
            MediaMetadata::Unknown(title.to_string())
        }
    };
    (metadata, name_guess)
}
/// process an entire collection for metadata
fn content_metadata(media: Vec<(path::PathBuf, ContentFileDetails)>) -> ContentScanResults {
    let mut content_name_guesser = HashMap::new();
    let mut content = Vec::new();
    let mut media_file_map = HashMap::new();
    for (path, e) in media {
        let (metadata, name_guess) = extract_metadata(e.title.as_str());
        if let Some(name) = name_guess {
            *content_name_guesser.entry(name).or_insert(0) += 1;
        }
        let content_data = ContentData {
            subtitle: e.subtitles,
            media_hash: e.media_hash,
            metadata,
        };
        media_file_map.insert(e.media_hash, path);
        content.push(content_data);
    }
    let content_name = content_name_guesser
        .into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(s, _)| s);

    ContentScanResults {
        suggested_name: content_name,
        files: media_file_map,
        content,
    }
}


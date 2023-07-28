use std::{io::Read, path};

use database::Database;

use super::{ScanError, ScannedData, ScannedSubtitles};
use crate::hashfs::compute_hash;

pub(crate) async fn read_media_from_path(
    db: &Database,
    media_path: &path::Path,
    trust_hashes: bool,
) -> anyhow::Result<ScannedData> {
    let subtitles = extract_subtitles(media_path)?;
    let media_hash = if trust_hashes {
        match db.get_storage_by_path(media_path).await {
            Ok(media_opt) => media_opt.map(|m| Ok(m.hash)),
            Err(e) => {
                log::error!(
                    "could not read hash for {:?} from db storage: {}",
                    media_path,
                    e
                );
                None
            }
        }
    } else {
        None
    };
    let media_hash = if let Some(hash) = media_hash {
        hash
    } else {
        compute_hash(media_path).await
    }?;

    Ok(ScannedData {
        path: media_path.to_path_buf(),
        subs: subtitles,
        hash: media_hash,
    })
}

/// find/extract subtitles for a given piece of media
fn extract_subtitles(media_path: &path::Path) -> anyhow::Result<ScannedSubtitles> {
    let srt_path = media_path.with_extension("srt");
    if !srt_path.exists() {
        return Ok(ScannedSubtitles::NotFound);
    }
    let srt_contents = read_path_to_string(srt_path.as_path())?;
    Ok(match subrip::parse(&srt_contents) {
        Ok(s) => ScannedSubtitles::Subtitles(s),
        Err(e) => ScannedSubtitles::Error(e),
    })
}

fn read_path_to_string<P: AsRef<path::Path>>(tpath: P) -> anyhow::Result<String> {
    let tpath = tpath.as_ref();
    let mut f = std::fs::File::open(tpath)?;
    let mut v = Vec::new();
    f.read_to_end(&mut v)?;

    Ok(match String::from_utf8(v) {
        Ok(s) => s,
        Err(e) => {
            let v = e.into_bytes();
            // SRT files are WINDOWS_1252 by default, but there is no requirement, so who knows
            let (text, encoding, replacements) = encoding_rs::WINDOWS_1252.decode(v.as_slice());
            if replacements {
                log::warn!(
                    "could not decode {:?} accurately with {}",
                    tpath,
                    encoding.name()
                );
            }
            text.to_string()
        }
    })
}

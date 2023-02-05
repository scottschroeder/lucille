use std::{io::Read, path};

use lucile_core::{
    hash::Sha2Hash,
    metadata::{MediaHash},
};
use sha2::{Digest, Sha256};

use super::{ScanError, ScannedData, ScannedSubtitles};

pub(crate) fn read_media_from_path(media_path: &path::Path) -> Result<ScannedData, ScanError> {
    let subtitles = extract_subtitles(media_path)?;
    let media_hash = hash_file(media_path)?;

    Ok(ScannedData {
        path: media_path.to_path_buf(),
        subs: subtitles,
        hash: media_hash,
    })
}

/// find/extract subtitles for a given piece of media
fn extract_subtitles(media_path: &path::Path) -> Result<ScannedSubtitles, ScanError> {
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

/// Get the sha2 hash for a media path
fn hash_file(fname: &path::Path) -> Result<MediaHash, ScanError> {
    let mut r = std::io::BufReader::new(std::fs::File::open(fname)?);
    let mut hasher = Sha256::new();

    // TODO FIXME DO NOT MERGE temp hack to speed up test cycle
    // let mut file = std::io::Cursor::new(p.as_os_str().as_bytes());

    std::io::copy(&mut r, &mut hasher)?;
    Ok(MediaHash::new(Sha2Hash::from(hasher.finalize())))
}

fn read_path_to_string<P: AsRef<path::Path>>(tpath: P) -> Result<String, ScanError> {
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

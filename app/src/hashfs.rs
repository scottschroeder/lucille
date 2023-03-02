use std::{path::PathBuf, str::FromStr};

use lucille_core::{hash::HashIo, metadata::MediaHash};
use tokio::io::{AsyncBufRead, AsyncRead};

const TMP_DIR: &str = ".tmp";

pub struct HashFS {
    root: PathBuf,
    tmp: PathBuf,
}

fn hash_path(hash: MediaHash) -> (String, String) {
    (
        format!("{:02x}/{:02x}", &hash.as_slice()[0], &hash.as_slice()[1],),
        hash.to_string(),
    )
}

/// Get the sha2 hash for a media path
pub(crate) async fn compute_hash(fname: &std::path::Path) -> Result<MediaHash, std::io::Error> {
    log::trace!("compute hash for {:?}", fname);
    let mut r = tokio::io::BufReader::new(tokio::fs::File::open(fname).await?);
    let mut hasher = HashIo::new(tokio::io::sink());
    tokio::io::copy(&mut r, &mut hasher).await?;
    let (_, hash) = hasher.into_inner();
    Ok(MediaHash::new(hash))
}

impl HashFS {
    pub fn new<P: Into<PathBuf>>(p: P) -> Result<HashFS, std::io::Error> {
        let root: PathBuf = p.into();
        let root = root.canonicalize()?;
        let tmp = root.join(TMP_DIR);
        std::fs::create_dir_all(&tmp)?;
        Ok(HashFS { root, tmp })
    }
    pub async fn reader(&self, hash: MediaHash) -> Result<impl AsyncBufRead, std::io::Error> {
        Ok(tokio::io::BufReader::new(
            tokio::fs::File::open(self.get_file_path(hash)).await?,
        ))
    }
    pub async fn write<R: AsyncRead + Unpin>(
        &self,
        reader: &mut R,
    ) -> Result<(PathBuf, MediaHash), std::io::Error> {
        let tf = tempfile::Builder::default().tempfile_in(self.tmp.as_path())?;
        let (std_file, tmp_path) = tf.into_parts();

        let f = tokio::fs::File::from_std(std_file);
        let mut hashed_file = HashIo::new(f);

        tokio::io::copy(reader, &mut hashed_file).await?;

        let (_, hash) = hashed_file.into_inner();
        let hash = MediaHash::new(hash);

        let (dname, fname) = self.get_path(hash);
        tokio::fs::create_dir_all(&dname).await?;
        let fpath = dname.join(fname);
        tokio::fs::rename(tmp_path.to_path_buf(), &fpath).await?;
        Ok((fpath, hash))
    }
    pub fn get_path(&self, hash: MediaHash) -> (PathBuf, String) {
        let (dir_name, file_name) = hash_path(hash);
        (self.root.join(dir_name), file_name)
    }
    pub fn get_file_path(&self, hash: MediaHash) -> PathBuf {
        let (d, f) = self.get_path(hash);
        d.join(f)
    }

    pub async fn remove(&self, hash: MediaHash) -> Result<(), std::io::Error> {
        let p = self.get_file_path(hash);
        log::trace!("rm {:?}", p);
        tokio::fs::remove_file(&p).await?;

        let mut d = p.as_path();
        while let Some(parent) = d.parent() {
            if parent == self.root {
                break;
            }
            if tokio::fs::remove_dir(parent).await.is_err() {
                break;
            } else {
                log::trace!("rmdir {:?}", parent);
                d = parent;
            }
        }
        Ok(())
    }

    pub async fn all_hashes(&self) -> Result<Vec<(PathBuf, MediaHash)>, std::io::Error> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || sync_all_hashes(&root))
            .await
            .expect("task did not complete")
    }
}

fn sync_all_hashes(root: &std::path::Path) -> Result<Vec<(PathBuf, MediaHash)>, std::io::Error> {
    let mut content = Vec::new();
    let entry_predicate = |entry: &walkdir::DirEntry| -> bool {
        entry.path() == root
            || !entry
                .file_name()
                .to_str()
                .map(|s| s.starts_with('.'))
                .unwrap_or(false)
    };
    for dir in walkdir::WalkDir::new(root)
        .into_iter()
        // .filter_entry(|e| !is_hidden(e))
        .filter_entry(entry_predicate)
    {
        let dir = dir?;
        if !dir.path().is_file() {
            continue;
        }
        let path = dir.path().to_owned();
        if let Some(hash) =
            path.file_name()
                .and_then(|os| os.to_str())
                .and_then(|s| match MediaHash::from_str(s) {
                    Ok(h) => Some(h),
                    Err(e) => {
                        log::warn!("HashFS could not parse hash from `{:?}`: {}", &path, e);
                        None
                    }
                })
        {
            content.push((path, hash));
        }
    }
    Ok(content)
}

#[cfg(test)]
mod test {
    use std::{collections::HashMap, io::Read, str::FromStr};

    use lucille_core::hash::Sha2Hash;
    use tokio::io::AsyncReadExt;

    use super::*;
    const TEST_DATA: &str = "the quick brown fox jumped over the lazy log\n";
    const TEST_HASH: &str = "e2291e7093575a6f3de282e558ee85b0eab2e8e1f1025c0f277a5ee31e4cfb84";
    #[test]
    fn make_dir_file_structure_from_hash() {
        let hash = MediaHash::new(Sha2Hash::from_str(TEST_HASH).unwrap());
        let (d, f) = hash_path(hash);
        assert_eq!(d, "e2/29");
        assert_eq!(f, TEST_HASH);
    }

    #[test]
    fn path_with_leading_zeros() {
        let input = b"13750\n";
        let hash_str = "0901fd30864ff2d77b1c54d4fe53a032bb4c193f73ca6f1241ae931414029892";
        let hash = MediaHash::from_bytes(input);

        let (d, f) = hash_path(hash);
        assert_eq!(f, hash_str);
        assert_eq!(d, "09/01");
    }

    #[tokio::test]
    async fn hashfs_write_file() {
        let root = tempfile::tempdir().unwrap();
        let expected_hash = MediaHash::new(Sha2Hash::from_str(TEST_HASH).unwrap());
        let hfs = HashFS::new(root.path()).unwrap();
        let mut source = std::io::Cursor::new(TEST_DATA);
        let (fpath, hash) = hfs.write(&mut source).await.unwrap();
        let expected_path = root.path().join(format!("e2/29/{}", TEST_HASH));

        assert_eq!(fpath, expected_path);
        assert_eq!(hash, expected_hash);
        assert!(expected_path.exists());

        let mut f = std::fs::File::open(expected_path).unwrap();
        let mut sink = String::new();
        f.read_to_string(&mut sink).unwrap();
        assert_eq!(sink, TEST_DATA);

        let mut dir_listing = std::fs::read_dir(root.path().join(TMP_DIR)).unwrap();
        assert!(dir_listing.next().is_none());
    }

    #[tokio::test]
    async fn get_listing_for_empty_hashfs() {
        let root = tempfile::tempdir().unwrap();
        let hfs = HashFS::new(root.path()).unwrap();
        let hashes = hfs.all_hashes().await.unwrap();
        assert!(hashes.is_empty());
    }

    #[tokio::test]
    async fn get_listing_for_single_entry() {
        let root = tempfile::tempdir().unwrap();
        let content = b"data1";
        let expected_hash = MediaHash::from_bytes(content);
        let mut rdr = std::io::Cursor::new(content);
        let hfs = HashFS::new(root.path()).unwrap();

        let (p1, h1) = hfs.write(&mut rdr).await.unwrap();
        assert_eq!(h1, expected_hash);

        let bytes = tokio::fs::read(&p1).await.unwrap();
        assert_eq!(&bytes, content);

        let hashes = hfs.all_hashes().await.unwrap();
        assert_eq!(hashes, vec![(p1, expected_hash)])
    }

    #[tokio::test]
    async fn get_listing_for_multiple_entries() {
        let root = tempfile::tempdir().unwrap();
        let hfs = HashFS::new(root.path()).unwrap();
        let mut expected = HashMap::new();
        let count = 50usize;
        for x in 0..count {
            let data = format!("data_{}", x);
            let mut rdr = std::io::Cursor::new(&data);
            let (p1, h1) = hfs.write(&mut rdr).await.unwrap();
            expected.insert(h1.to_string(), (data, p1, h1));
        }

        let hashes = hfs.all_hashes().await.unwrap();
        assert_eq!(hashes.len(), count);
        for (actual_path, actual_hash) in hashes {
            let (e_data, e_path, e_hash) = &expected[&actual_hash.to_string()];
            assert_eq!(&actual_hash, e_hash);
            assert_eq!(&actual_path, e_path);
            let bytes = tokio::fs::read(&actual_path).await.unwrap();
            assert_eq!(&bytes, e_data.as_bytes());
        }
    }

    #[tokio::test]
    async fn remove_single_entry() {
        let root = tempfile::tempdir().unwrap();
        let hfs = HashFS::new(root.path()).unwrap();

        let mut rdr1 = std::io::Cursor::new(b"data1");
        let (p1, h1) = hfs.write(&mut rdr1).await.unwrap();
        let mut rdr2 = std::io::Cursor::new(b"data2");
        let (p2, _h2) = hfs.write(&mut rdr2).await.unwrap();

        hfs.remove(h1).await.unwrap();
        assert!(!p1.exists());
        assert!(p2.exists());
    }

    #[tokio::test]
    async fn remove_entries_with_shared_parents() {
        /*
         * This test uses three specifically chosen hashes to verify
         * that removing a file will remove as many parents as possible
         * all three share `./de` and two share `./de/ad`
         *  1621: de690d1ae70d10081585d8ed98ed5825ac88fe8029b67a583a760fcc1d505636
         *  109583: deadc19bb1cd5f49f9783b1f8cacd788e5fb7646264307f34041609dd71473b9
         *  146786: dead536238eeae54d8205a34c59218c502fd5c53a468eb4069eedd3332cf1f5f
         */
        let root = tempfile::tempdir().unwrap();
        let hfs = HashFS::new(root.path()).unwrap();

        let mut rdr1 = std::io::Cursor::new(b"1621");
        let (p1, h1) = hfs.write(&mut rdr1).await.unwrap();
        let mut rdr2 = std::io::Cursor::new(b"109583");
        let (p2, h2) = hfs.write(&mut rdr2).await.unwrap();
        let mut rdr3 = std::io::Cursor::new(b"146786");
        let (p3, h3) = hfs.write(&mut rdr3).await.unwrap();

        assert!(p1.exists());
        assert!(p2.exists());
        assert!(p3.exists());
        assert!(root.path().join("de").exists());
        assert!(root.path().join("de/ad").exists());

        hfs.remove(h3).await.unwrap();
        assert!(p1.exists());
        assert!(p2.exists());
        assert!(!p3.exists());
        assert!(root.path().join("de").exists());
        assert!(root.path().join("de/ad").exists());

        hfs.remove(h2).await.unwrap();
        assert!(p1.exists());
        assert!(!p2.exists());
        assert!(!p3.exists());
        assert!(root.path().join("de").exists());
        assert!(!root.path().join("de/ad").exists());

        hfs.remove(h1).await.unwrap();
        assert!(!p1.exists());
        assert!(!p2.exists());
        assert!(!p3.exists());
        assert!(!root.path().join("de").exists());
        assert!(!root.path().join("de/ad").exists());
    }
}

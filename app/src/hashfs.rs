use std::path::PathBuf;

use lucile_core::{hash::HashIo, metadata::MediaHash};
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
}

#[cfg(test)]
mod test {
    use std::{io::Read, str::FromStr};

    use lucile_core::hash::Sha2Hash;

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
}

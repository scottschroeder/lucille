use super::index::Uuid;
use anyhow::Result;
use nom::AsBytes;
use std::{
    collections::HashMap,
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

pub trait Storage {
    // TODO async
    fn get_bytes(&self, id: Uuid) -> Result<Option<Vec<u8>>>;
    fn insert_bytes(&self, id: Uuid, data: &[u8]) -> Result<()>;
    fn insert_file<P: AsRef<Path>>(&self, id: Uuid, p: P) -> Result<()> {
        let mut data = Vec::new();
        let mut f = std::fs::File::open(p)?;
        f.read_to_end(&mut data)?;
        self.insert_bytes(id, data.as_bytes())?;
        Ok(())
    }
}

pub struct FileStorage {
    root: PathBuf,
}

impl FileStorage {
    pub fn new<P: AsRef<Path>>(p: P) -> Result<FileStorage> {
        let root = p.as_ref().to_owned();
        if !root.exists() {
            std::fs::create_dir_all(root.as_path())?;
        }
        Ok(FileStorage {
            root,
        })
    }
}

impl Storage for FileStorage {
    fn get_bytes(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        let p = self.root.join(id.to_string());
        match std::fs::File::open(p) {
            Ok(f) => {
                let mut buf = Vec::new();
                BufReader::new(f).read_to_end(&mut buf)?;
                Ok(Some(buf))
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => Ok(None),
                _ => Err(e.into()),
            },
        }
    }
    fn insert_bytes(&self, id: Uuid, data: &[u8]) -> Result<()> {
        let p = self.root.join(id.to_string());
        let mut f = std::fs::File::create(p)?;
        f.write_all(data)?;
        Ok(())
    }
}

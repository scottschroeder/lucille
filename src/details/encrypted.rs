/*!
    WARNING!!!

    This encryption is not meant to be secure IN THE SLIGHTEST.

    I just wanted 6ft of distance between copyrighted material
    and a storage backend that wasn't mine. I could have
    just XOR'd the bytes with 01010101, but maybe in the future
    we actually DO want to implement real encryption.
*/

use crate::details::index::Uuid;
use crate::details::index::VideoSegmentId;
use super::storage::Storage;
use std::collections::HashMap;
use anyhow::Result;

struct EncryptedVideoSegmentId(Uuid);



struct EncryptedSegments {
    inner: HashMap<VideoSegmentId, EncryptedVideoSegmentId>,
    keyname: String,
}

trait KeyFetcher {
    fn get(&self, name: &str) -> Result<Option<String>>;
}

struct EncryptedStorage<S, T> {
    key_fetcher: T,
    storage: S
}

impl<S: Storage, T: KeyFetcher> Storage for EncryptedStorage<S, T> {
    fn get_bytes(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        self.storage.get_bytes(id)
    }

    fn insert_bytes(&self, id: Uuid, data: &[u8]) -> Result<()> {
        self.storage.insert_bytes(id, data)
    }
}
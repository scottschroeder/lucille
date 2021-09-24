/*!
    WARNING!!!

    This encryption is not meant to be secure IN THE SLIGHTEST.

    I just wanted 6ft of distance between copyrighted material
    and a storage backend that wasn't mine. I could have
    just XOR'd the bytes with 01010101, but maybe in the future
    we actually DO want to implement real encryption.
*/

use super::storage::Storage;
use crate::content::index::Uuid;
use anyhow::Result;
use sodiumoxide::crypto::secretstream::{Header, Key, Stream, Tag, HEADERBYTES};

struct EncryptedStorage<S> {
    key: Key,
    storage: S,
}

impl<S: Storage> Storage for EncryptedStorage<S> {
    fn get_bytes(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        if let Some(buf) = self.storage.get_bytes(id)? {
            let header = Header::from_slice(&buf[0..HEADERBYTES]).unwrap();
            let mut s = Stream::init_pull(&header, &self.key)
                .map_err(|_| anyhow::anyhow!("invalid header: {:?}", header))?;
            let (v, _) = s
                .pull(&buf[HEADERBYTES..], None)
                .map_err(|_| anyhow::anyhow!("invalid ciphertext"))?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    fn insert_bytes(&self, id: Uuid, data: &[u8]) -> Result<()> {
        let (mut s, header) = Stream::init_push(&self.key).expect("unable to create cipher");
        let mut enc_data = header.as_ref().to_vec();
        s.push_to_vec(data, None, Tag::Message, &mut enc_data)
            .expect("unable to encrypt data");
        enc_data.extend(s.finalize(None).expect("unable to finalize stream"));

        self.storage.insert_bytes(id, enc_data.as_slice())
    }
}

/*!
    WARNING!!!

    This encryption is not meant to be secure IN THE SLIGHTEST.

    I just wanted 6ft of distance between copyrighted material
    and a storage backend that wasn't mine. I could have
    just XOR'd the bytes with 01010101, but maybe in the future
    we actually DO want to implement real encryption.
*/

use super::storage::Storage;
use crate::details::index::Uuid;
use anyhow::Result;
use sodiumoxide::crypto::secretstream::{gen_key, Header, Key, Stream, Tag, HEADERBYTES};

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

// pub(crate) mod aesbytes {
//     use aes::{
//         cipher::{
//             generic_array::GenericArray, BlockCipher, BlockDecrypt, BlockEncrypt, NewBlockCipher,
//         },
//         Aes128, Aes256, Block, ParBlocks
//     };

//     type AesMode = Aes256;
//     type KeySize = <AesMode as NewBlockCipher>::KeySize;

//     struct Key<'a>(&'a GenericArray<u8, KeySize>);

//     impl<'a> Key<'a> {
//         pub fn new(s: &str) -> Key {
//             Key(GenericArray::from_slice(s.as_bytes()))
//         }
//     }

//     // pub fn encrypt(key: &Key) {
//     pub fn encrypt(key: &str) {
//         let key = Key::new(key);
//         let mut block = Block::default();

//         // Initialize cipher
//         let cipher = AesMode::new(&key.0);

//         let block_copy = block.clone();
//         log::trace!("b pre: {:?}", block);

//         // Encrypt block in-place
//         cipher.encrypt_block(&mut block);
//         log::trace!("b enc: {:?}", block);

//         // And decrypt it back
//         cipher.decrypt_block(&mut block);
//         log::trace!("b dec: {:?}", block);
//         assert_eq!(block, block_copy);

//         // We can encrypt 8 blocks simultaneously using
//         // instruction-level parallelism
//         let mut block8 = ParBlocks::default();
//         let block8_copy = block8.clone();
//         log::trace!("b8 pre: {:?}", block8);
//         cipher.encrypt_par_blocks(&mut block8);
//         log::trace!("b8 enc: {:?}", block8);
//         cipher.decrypt_par_blocks(&mut block8);
//         log::trace!("b8 dec: {:?}", block8);
//         assert_eq!(block8, block8_copy);
//     }

// }

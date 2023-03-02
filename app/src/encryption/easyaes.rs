use aes_gcm::{self, aead::Aead, Aes128Gcm, KeyInit};
use lucille_core::encryption_config::{KeyData, SimpleKeyNonce as MessageMeta};
use rand::Rng;

use super::EncryptionError;

const NONCE_LENGTH: usize = 12;

pub(crate) fn scramble(plaintext: &[u8]) -> Result<(KeyData, Vec<u8>), EncryptionError> {
    let mut rng = rand::thread_rng();
    let key = Aes128Gcm::generate_key(&mut rng);
    let cipher = Aes128Gcm::new(&key);
    let mut nonce = vec![0; NONCE_LENGTH];
    rng.fill(nonce.as_mut_slice());
    let ciphertext = cipher.encrypt(nonce.as_slice().into(), plaintext)?;
    let key = key.to_vec();
    Ok((
        KeyData::EasyAesGcmInMemory(MessageMeta { key, nonce }),
        ciphertext,
    ))
}

pub fn unscramble(ciphertext: &[u8], meta: &MessageMeta) -> Result<Vec<u8>, EncryptionError> {
    let cipher = Aes128Gcm::new_from_slice(meta.key.as_slice())?;
    let plaintext = cipher.decrypt(meta.nonce.as_slice().into(), ciphertext)?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check() {
        let plain = "this is an example bit of data";
        let (keydata, cipher) = scramble(plain.as_bytes()).unwrap();
        let meta = match keydata {
            KeyData::EasyAesGcmInMemory(meta) => meta,
            _ => panic!("incorrect type of keydata"),
        };
        let decrypted = unscramble(&cipher, &meta).unwrap();
        assert_eq!(decrypted, plain.as_bytes())
    }
}

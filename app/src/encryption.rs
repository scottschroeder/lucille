use aes_gcm::aes::cipher::InvalidLength;
use tokio::io::AsyncReadExt;
pub(crate) mod easyaes;

pub use easyaes::unscramble;
use lucille_core::encryption_config::KeyData;

#[derive(Debug, thiserror::Error)]
#[deprecated(note = "use anyhow")]
pub enum EncryptionError {
    #[error(transparent)]
    Aead(#[from] aes_gcm::Error),
    #[error(transparent)]
    InvalidLength(#[from] InvalidLength),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub async fn decryptor<T: tokio::io::AsyncRead + Unpin>(
    cfg: &KeyData,
    reader: &mut T,
) -> Result<Box<dyn tokio::io::AsyncRead + Unpin + Send>, EncryptionError> {
    match cfg {
        KeyData::EasyAesGcmInMemory(key_nonce) => {
            // TODO make an async decryptor.
            // Right now we just read everything into a buffer and pretend
            // we give back a reader.
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            let plaintext = unscramble(buf.as_slice(), key_nonce)?;
            Ok(Box::new(std::io::Cursor::new(plaintext)))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn encrypt_decrypt_easy_aes() {
        let input_text = "MY SECRET DATA".repeat(50);
        let (keydata, ciphertext) = easyaes::scramble(input_text.as_bytes()).unwrap();
        let mut cipher_reader = std::io::Cursor::new(ciphertext);
        let mut plain_reader = decryptor(&keydata, &mut cipher_reader).await.unwrap();
        let mut s = String::new();
        plain_reader.read_to_string(&mut s).await.unwrap();
        assert_eq!(s, input_text);
    }
}

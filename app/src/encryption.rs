use aes_gcm::aes::cipher::InvalidLength;
use tokio::io::AsyncReadExt;
pub(crate) mod easyaes;

pub use easyaes::unscramble;
use lucile_core::encryption_config::KeyData;

#[derive(Debug, thiserror::Error)]
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
) -> Result<Box<dyn tokio::io::AsyncRead + Unpin>, EncryptionError> {
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

use std::str::FromStr;

use aes_gcm::aes::cipher::InvalidLength;
use serde::{Deserialize, Serialize};
pub(crate) mod easyaes;

pub use easyaes::unscramble;

#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error(transparent)]
    Aead(#[from] aes_gcm::Error),
    #[error(transparent)]
    InvalidLength(#[from] InvalidLength),
    #[error(transparent)]
    DecodeError(#[from] base64::DecodeError),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum KeyData {
    EasyAesGcmInMemory(easyaes::MessageMeta),
}

impl FromStr for KeyData {
    type Err = EncryptionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let data = base64::decode(s)?;
        let key: Self = serde_json::from_slice(&data)?;
        Ok(key)
    }
}

impl std::fmt::Debug for KeyData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EasyAesGcmInMemory(_arg0) => f
                .debug_tuple("EasyAesGcmInMemory")
                .field(&KeyDataB64(self))
                .finish(),
        }
    }
}

struct KeyDataB64<'a>(&'a KeyData);
impl<'a> std::fmt::Debug for KeyDataB64<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl<'a> std::fmt::Display for KeyDataB64<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json_str = serde_json::to_string(self.0).map_err(|e| {
            log::error!("unable to format key {:?}: {}", self.0, e);
            std::fmt::Error
        })?;
        let b64_wrapper = base64::display::Base64Display::new(
            json_str.as_bytes(),
            &base64::engine::general_purpose::STANDARD,
        );
        write!(f, "{}", b64_wrapper)
    }
}

mod serde_base64 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        let base64 = base64::encode(v);
        String::serialize(&base64, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let base64 = String::deserialize(d)?;
        base64::decode(base64.as_bytes()).map_err(|e| serde::de::Error::custom(e))
    }
}

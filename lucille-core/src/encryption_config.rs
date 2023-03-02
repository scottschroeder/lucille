use std::{fmt, str::FromStr};

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::base64::{serde_base64, B64Bytes};

#[derive(Debug, thiserror::Error)]
pub enum EncryptionConfigError {
    #[error(transparent)]
    DecodeError(#[from] base64::DecodeError),
    #[error(transparent)]
    JsonError(#[from] serde_json::Error),
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct SimpleKeyNonce {
    #[serde(with = "serde_base64")]
    pub key: Vec<u8>,
    #[serde(with = "serde_base64")]
    pub nonce: Vec<u8>,
}

impl fmt::Debug for SimpleKeyNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimpleKeyNonce")
            .field("key", &B64Bytes(self.key.as_slice()))
            .field("nonce", &B64Bytes(self.nonce.as_slice()))
            .finish()
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum KeyData {
    EasyAesGcmInMemory(SimpleKeyNonce),
}

impl fmt::Debug for KeyData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EasyAesGcmInMemory(_arg0) => f
                .debug_tuple("EasyAesGcmInMemory")
                .field(&KeyDataB64(self))
                .finish(),
        }
    }
}

impl fmt::Display for KeyData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", KeyDataB64(self))
    }
}

struct KeyDataB64<'a>(&'a KeyData);

impl<'a> fmt::Debug for KeyDataB64<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl<'a> fmt::Display for KeyDataB64<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json_str = serde_json::to_string(self.0).map_err(|e| {
            log::error!("unable to format key {:?}: {}", self.0, e);
            fmt::Error
        })?;
        write!(f, "{}", B64Bytes(json_str.as_bytes()))
    }
}
impl FromStr for KeyData {
    type Err = EncryptionConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let data = crate::base64::B64.decode(s)?;
        let key: Self = serde_json::from_slice(&data)?;
        Ok(key)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const B64_EASY_AES_GCM_IN_MEMORY: &str =
        "eyJFYXN5QWVzR2NtSW5NZW1vcnkiOnsia2V5IjoiQVFJREJBVT0iLCJub25jZSI6IkNBa0sifX0=";

    fn easy_gcm_in_memory() -> KeyData {
        KeyData::EasyAesGcmInMemory(SimpleKeyNonce {
            key: vec![1, 2, 3, 4, 5],
            nonce: vec![8, 9, 10],
        })
    }

    #[test]
    fn serialize_easy_aes() {
        let s = format!("{}", easy_gcm_in_memory());
        assert_eq!(s, B64_EASY_AES_GCM_IN_MEMORY);
    }

    #[test]
    fn deserialize_easy_aes() {
        let cfg = B64_EASY_AES_GCM_IN_MEMORY.parse::<KeyData>().unwrap();
        assert_eq!(cfg, easy_gcm_in_memory());
    }
}

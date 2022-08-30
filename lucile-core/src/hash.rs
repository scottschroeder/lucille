use hex::FromHex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::Digest;
use std::{fmt, str::FromStr};

const HASH_SIZE: usize = 32;
type MySha = sha2::Sha256;
type HashBytes = [u8; HASH_SIZE];

#[derive(PartialEq, Clone, Copy, Eq, Hash)]
pub struct Sha2Hash(HashBytes);

impl Sha2Hash {
    pub fn encode(&self) -> String {
        hex::encode(self.0)
    }
}

impl Sha2Hash {
    pub fn digest<T: AsRef<[u8]>>(data: T) -> Self {
        Sha2Hash(MySha::digest(data.as_ref()).into())
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl<T: Into<HashBytes>> From<T> for Sha2Hash {
    fn from(inner: T) -> Sha2Hash {
        Sha2Hash(inner.into())
    }
}

impl FromStr for Sha2Hash {
    type Err = hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex_bytes = if s.starts_with("0x") || s.starts_with("0X") {
            &s.as_bytes()[2..]
        } else {
            &s.as_bytes()[..]
        };
        Ok(Sha2Hash::from(<HashBytes>::from_hex(hex_bytes)?))
    }
}

impl fmt::Debug for Sha2Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", self)
    }
}

impl fmt::Display for Sha2Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.encode())
    }
}

impl Serialize for Sha2Hash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

impl<'de> Deserialize<'de> for Sha2Hash {
    fn deserialize<D>(deserializer: D) -> Result<Sha2Hash, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use serde_json;

    const TEST_DATA: &str = "the quick brown fox jumped over the lazy log\n";
    const TEST_HASH_STR: &str = "e2291e7093575a6f3de282e558ee85b0eab2e8e1f1025c0f277a5ee31e4cfb84";

    fn hash_test_data() -> Sha2Hash {
        Sha2Hash::digest(TEST_DATA)
    }

    #[test]
    fn hash_debug_write() {
        assert_eq!(
            format!("{:?}", hash_test_data()),
            format!("0x{}", TEST_HASH_STR)
        );
    }

    #[test]
    fn hash_display_write() {
        assert_eq!(
            format!("{}", hash_test_data()),
            format!("{}", TEST_HASH_STR)
        );
    }

    #[test]
    fn hash_serialize() {
        assert_eq!(
            format!("{}", serde_json::to_string(&hash_test_data()).unwrap()),
            format!("\"0x{}\"", TEST_HASH_STR)
        );
    }

    #[test]
    fn hash_deserialize() {
        assert_eq!(
            serde_json::from_str::<Sha2Hash>(&format!("\"0x{}\"", TEST_HASH_STR)).unwrap(),
            hash_test_data(),
        );
    }

    #[test]
    fn hash_serde() {
        let orig =
            serde_json::from_str::<Sha2Hash>(&serde_json::to_string(&hash_test_data()).unwrap())
                .unwrap();
        assert_eq!(orig, hash_test_data(),);
    }

    #[test]
    fn parse_hash_str_w_0x() {
        assert_eq!(
            hash_test_data(),
            Sha2Hash::from_str(&format!("0x{}", TEST_HASH_STR)).unwrap()
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn parse_hash_str_w_0X() {
        assert_eq!(
            hash_test_data(),
            Sha2Hash::from_str(&format!("0X{}", TEST_HASH_STR)).unwrap()
        );
    }

    #[test]
    fn parse_hash_str_wo_header() {
        assert_eq!(hash_test_data(), Sha2Hash::from_str(TEST_HASH_STR).unwrap());
    }

    #[test]
    fn fail_short_parse() {
        match Sha2Hash::from_str("2342342342342adf") {
            Ok(h) => panic!("incorrectly parsed: {:?}", h),
            Err(hex::FromHexError::InvalidStringLength) => {}
            Err(e) => panic!("incorrect hex parse error: {}", e),
        }
    }

    #[test]
    fn fail_long_parse() {
        match Sha2Hash::from_str(
            "e2291e7093575a6f3de282e558ee85b0eab2e8e1f1025c0f277a5ee31e4cfb84deadbeef",
        ) {
            Ok(h) => panic!("incorrectly parsed: {:?}", h),
            Err(hex::FromHexError::InvalidStringLength) => {}
            Err(e) => panic!("incorrect hex parse error: {}", e),
        }
    }
    #[test]
    fn fail_invalid_str() {
        match Sha2Hash::from_str("e2291e7093575a6f3de282e558ee85b0eab2e8e1f1025c0f277a5ee31e4cfb8x")
        {
            Ok(h) => panic!("incorrectly parsed: {:?}", h),
            Err(hex::FromHexError::InvalidHexCharacter { c, index }) => {
                assert_eq!('x', c);
                assert_eq!(63, index);
            }
            Err(e) => panic!("incorrect hex parse error: {}", e),
        }
    }
}

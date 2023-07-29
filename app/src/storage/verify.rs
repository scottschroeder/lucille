use std::ffi::OsStr;

use database::Database;
use lucille_core::metadata::MediaHash;

use crate::hashfs::compute_hash;

/// When checking local files, this enum describes
/// how carefully to verify integrity
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileCheckStrategy {
    /// Verify all files by re-calculating the hash
    VerifyAll,
    /// If the filename matches the expected hash,
    /// skip re-calculating the full hash
    TrustNameIsHash,
    /// Only check that the file exists, do not verify hashes
    CheckExists,
}

/// The outcome of checking the file
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileCheckOutcome {
    /// File does not exist on the local filesystem
    DoesNotExist,

    /// File Exists
    Exists,

    /// File Verified
    Verified,

    /// File Exists, but the data does not match expected hash
    Invalid,
}

impl FileCheckOutcome {
    pub fn as_bool(self) -> bool {
        match self {
            FileCheckOutcome::DoesNotExist => false,
            FileCheckOutcome::Exists => true,
            FileCheckOutcome::Verified => true,
            FileCheckOutcome::Invalid => false,
        }
    }
}

pub async fn check_local_file(
    db: &Database,
    hash: MediaHash,
    strategy: FileCheckStrategy,
) -> anyhow::Result<Option<(std::path::PathBuf, FileCheckOutcome)>> {
    let file_meta = match db.get_storage_by_hash(hash).await? {
        Some(f) => f,
        None => return Ok(None),
    };

    let local_path = file_meta.path.as_path();

    if tokio::fs::metadata(local_path).await.is_err() {
        return Ok(Some((file_meta.path, FileCheckOutcome::DoesNotExist)));
    }

    match strategy {
        FileCheckStrategy::VerifyAll => {}
        FileCheckStrategy::TrustNameIsHash => {
            if let Some(fname) = local_path.file_name() {
                let hash_str = hash.to_string();
                if fname == OsStr::new(&hash_str) {
                    return Ok(Some((file_meta.path, FileCheckOutcome::Exists)));
                }
            }
        }
        FileCheckStrategy::CheckExists => {
            return Ok(Some((file_meta.path, FileCheckOutcome::Exists)))
        }
    }

    let actual_hash = compute_hash(local_path).await?;
    Ok(Some((
        file_meta.path,
        if actual_hash == hash {
            FileCheckOutcome::Verified
        } else {
            FileCheckOutcome::Invalid
        },
    )))
}

#[cfg(test)]
mod test {

    use tokio::io::AsyncWriteExt;

    use super::*;
    use crate::app::tests::lucille_test_app;

    struct TestCase {
        name_is_hash: bool,
        data_matches_hash: bool,
        file_exists: bool,
        strategy: FileCheckStrategy,
        expected: FileCheckOutcome,
    }

    macro_rules! check_test_case {
        ($($name:ident: $value:expr,)*) => {
            $(
                #[tokio::test]
                async fn $name() {
                    check_local_file_conditions($value).await;
                }
            )*
        }
    }

    async fn check_local_file_conditions(test_case: TestCase) {
        let TestCase {
            name_is_hash,
            data_matches_hash,
            file_exists,
            strategy,
            expected,
        } = test_case;

        let tapp = lucille_test_app().await;
        let dir = tempfile::TempDir::new().unwrap();

        let expected_hash = MediaHash::from_bytes(b"data_expected");
        let fname = if name_is_hash {
            dir.path().join(expected_hash.to_string())
        } else {
            dir.path().join("test-name")
        };

        if file_exists {
            let data = if data_matches_hash {
                "data_expected".as_bytes()
            } else {
                "data_unexpected".as_bytes()
            };
            let mut f = tokio::fs::File::create(&fname).await.unwrap();
            f.write_all(data).await.unwrap();
        }

        tapp.app
            .db
            .add_storage(expected_hash, &fname)
            .await
            .unwrap();

        let (_, actual) = check_local_file(&tapp.app.db, expected_hash, strategy)
            .await
            .expect("problem checking file on disk")
            .expect("hash not in db");
        assert_eq!(actual, expected);
    }

    check_test_case!(
        file_exists: TestCase {
            name_is_hash: false,
            data_matches_hash: true,
            file_exists: true,
            strategy: FileCheckStrategy::CheckExists,
            expected: FileCheckOutcome::Exists,
        },
        file_trust_hash: TestCase {
            name_is_hash: false,
            data_matches_hash: true,
            file_exists: true,
            strategy: FileCheckStrategy::TrustNameIsHash,
            expected: FileCheckOutcome::Verified,
        },
        file_verify: TestCase {
            name_is_hash: false,
            data_matches_hash: true,
            file_exists: true,
            strategy: FileCheckStrategy::VerifyAll,
            expected: FileCheckOutcome::Verified,
        },
        file_trust_hash_invalid: TestCase {
            name_is_hash: false,
            data_matches_hash: false,
            file_exists: true,
            strategy: FileCheckStrategy::TrustNameIsHash,
            expected: FileCheckOutcome::Invalid,
        },
        file_verify_invalid: TestCase {
            name_is_hash: false,
            data_matches_hash: false,
            file_exists: true,
            strategy: FileCheckStrategy::VerifyAll,
            expected: FileCheckOutcome::Invalid,
        },
        missing_file: TestCase {
            name_is_hash: false,
            data_matches_hash: true,
            file_exists: false,
            strategy: FileCheckStrategy::VerifyAll,
            expected: FileCheckOutcome::DoesNotExist,
        },
        hash_name_check_exists: TestCase {
            name_is_hash: true,
            data_matches_hash: true,
            file_exists: true,
            strategy: FileCheckStrategy::CheckExists,
            expected: FileCheckOutcome::Exists,
        },
        hash_name_trust: TestCase {
            name_is_hash: true,
            data_matches_hash: true,
            file_exists: true,
            strategy: FileCheckStrategy::TrustNameIsHash,
            expected: FileCheckOutcome::Exists,
        },
        hash_name_trust_but_its_wrong: TestCase {
            name_is_hash: true,
            data_matches_hash: false,
            file_exists: true,
            strategy: FileCheckStrategy::TrustNameIsHash,
            expected: FileCheckOutcome::Exists,
        },
        hash_name_verify: TestCase {
            name_is_hash: true,
            data_matches_hash: true,
            file_exists: true,
            strategy: FileCheckStrategy::VerifyAll,
            expected: FileCheckOutcome::Verified,
        },
        hash_name_verify_but_its_wrong: TestCase {
            name_is_hash: true,
            data_matches_hash: false,
            file_exists: true,
            strategy: FileCheckStrategy::VerifyAll,
            expected: FileCheckOutcome::Invalid,
        },
    );
}

use std::fmt;
use tantivy::TantivyError;
use thiserror::Error;

#[derive(Error, Debug)]
pub struct TError(TantivyError);

impl From<TantivyError> for TError {
    fn from(e: TantivyError) -> Self {
        TError(e)
    }
}

impl fmt::Display for TError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tantivy error: {}", self.0)
    }
}

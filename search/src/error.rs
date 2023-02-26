use tantivy::TantivyError;
use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]
pub struct TError(#[from] TantivyError);

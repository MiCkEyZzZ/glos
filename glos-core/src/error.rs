use glos_types::GlosError;
use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Glos format error: {0}")]
    Glos(#[from] GlosError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Unexpected and of input")]
    UnexpectedEof,

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

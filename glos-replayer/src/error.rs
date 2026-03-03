use glos_types::GlosError;
use thiserror::Error;

pub type ReplayResult<T> = Result<T, ReplayError>;

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Glos format error: {0}")]
    Glos(#[from] GlosError),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Timing error: {0}")]
    Timing(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

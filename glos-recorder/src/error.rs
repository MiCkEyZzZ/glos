use glos_types::GlosError;
use thiserror::Error;

pub type RecorderResult<T> = std::result::Result<T, RecorderError>;

#[derive(Debug, Error)]
pub enum RecorderError {
    #[error("SDR device not found: {0}")]
    DeviceNotFound(String),

    #[error("SDR device error: {0}")]
    DeviceError(String),

    #[error("Ring buffer overflow: {dropped} samples dropped in last batch")]
    BufferOverflow { dropped: u64 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("GLOS error: {0}")]
    Glos(#[from] GlosError),

    #[error("Pipeline error: {0}")]
    Pipeline(String),

    #[error("Duration limit reached")]
    DurationElapsed,
}

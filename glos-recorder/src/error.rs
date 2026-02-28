use thiserror::Error;

pub type RecorderResult<T> = std::result::Result<T, RecorderError>;

#[derive(Debug, Error)]
pub enum RecorderError {
    /// SDR устройство не найдено
    #[error("SDR device not found: {0}")]
    DeviceNotFound(String),

    /// Ошибка SDR устройства
    #[error("SDR device error: {0}")]
    DeviceError(String),

    /// Переполнение кольцевого буфера (producer быстрее consumer)
    #[error("Ring buffer overflow: {dropped} samples dropped in last batch")]
    BufferOverflow { dropped: u64 },

    /// Ошибка записи файла
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Ошибка формата GLOS
    #[error("GLOS error: {0}")]
    Glos(#[from] glos_core::error::GlosError),

    /// Ошибка пайплайна (inter-thread)
    #[error("Pipeline error: {0}")]
    Pipeline(String),

    /// Запись завершена по истечению времени
    #[error("Duration limit reached")]
    DurationElapsed,
}

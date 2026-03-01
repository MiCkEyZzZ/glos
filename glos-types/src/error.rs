use thiserror::Error;

/// Результат для операций GLOS
pub type GlosResult<T> = std::result::Result<T, GlosError>;

/// Типы ошибок формата GLOS.
#[derive(Debug, Error)]
pub enum GlosError {
    /// Неправильное магическое число
    #[error("Invalid magic: {0}")]
    InvalidMagic(String),

    /// Несовместимая версия формата
    #[error("Unsupported version: found {found}, expected {expected}")]
    UnsupportedVersion { found: u8, expected: u8 },

    /// Несовпадение CRC32 (ожидалось/найдено)
    #[error("CRC mismatch: expected {expected:08x}, found {found:08x}")]
    CrcMismatch { expected: u32, found: u32 },

    /// Повреждённые или некорректные данные
    #[error("Corrupted data: {0}")]
    Corrupted(String),

    /// Некорректный размер блока
    #[error("Invalid block size: {0}")]
    InvalidBlockSize(usize),

    /// Ошибки ввода/вывода (автоконвертируются из std::io::Error)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Нарушение спецификации формата
    #[error("Format violation: {0}")]
    FormatViolation(String),
}

impl GlosError {
    /// Удобные конструкторы
    pub fn invalid_magic<S: Into<String>>(s: S) -> Self {
        Self::InvalidMagic(s.into())
    }

    pub fn corrupted<S: Into<String>>(s: S) -> Self {
        Self::Corrupted(s.into())
    }

    pub fn format_violation<S: Into<String>>(s: S) -> Self {
        Self::FormatViolation(s.into())
    }
}

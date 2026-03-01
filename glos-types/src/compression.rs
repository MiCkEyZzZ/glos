use crate::{GlosError, GlosResult};

/// Тип сжатия IQ данных
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    /// Без сжатия
    None = 0,
    /// Сжатие LZ4
    Lz4 = 1,
}

impl Compression {
    pub fn from_u8(v: u8) -> GlosResult<Self> {
        match v {
            0 => Ok(Compression::None),
            1 => Ok(Compression::Lz4),
            _ => Err(GlosError::FormatViolation(format!(
                "Unknown compression: {v}"
            ))),
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

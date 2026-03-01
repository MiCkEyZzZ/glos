use crate::{GlosError, GlosResult};

/// Формат IQ выборок
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IqFormat {
    /// 8-битные целые числа (I8, Q8) — компактно
    Int8 = 0,
    /// 16-битные целые числа (I16, Q16) — выше точность
    Int16 = 1,
    /// 32-битные числа с плавающей точкой (F32, F32) — полная точность
    Float32 = 2,
}

impl IqFormat {
    pub fn from_u8(v: u8) -> GlosResult<Self> {
        match v {
            0 => Ok(IqFormat::Int8),
            1 => Ok(IqFormat::Int16),
            2 => Ok(IqFormat::Float32),
            _ => Err(GlosError::FormatViolation(format!(
                "Unknown IQ format: {v}"
            ))),
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    /// Размер одной IQ пары в байтах
    pub fn sample_size(&self) -> usize {
        match self {
            IqFormat::Int8 => 2,    // 1 байт I + 1 байт Q
            IqFormat::Int16 => 4,   // 2 байта I + 2 байта Q
            IqFormat::Float32 => 8, // 4 байта I + 4 байта Q
        }
    }
}

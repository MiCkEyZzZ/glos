use crate::{Compression, IqFormat, SdrType};

/// Заголовок GLOS файла (фиксированный размер 128 байт)
#[derive(Debug, Clone)]
pub struct GlosHeader {
    /// Версия формата ГЛОС
    pub version: u8,
    /// Флаги (bit 0: little-endian если установлен)
    pub flags: u8,
    /// Тип SDR устройства
    pub sdr_type: SdrType,
    /// Формат IQ данных
    pub iq_format: IqFormat,
    /// Метод сжатия
    pub compression: Compression,
    /// Частота дискретизации в Гц
    pub sample_rate: u32,
    /// Несущая частота в Гц
    pub center_freq: u64,
    /// Усиление приёмника в дБ (f32)
    pub gain_db: f32,
    /// Время начала сессии (Unix timestamp, секунды)
    pub timestamp_start: u64,
    /// Время окончания сессии (0 если запись продолжается)
    pub timestamp_end: u64,
    /// Общее количество IQ выборок в файле
    pub total_samples: u64,
}

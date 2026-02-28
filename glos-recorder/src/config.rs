use std::path::PathBuf;

use glos_core::{Compression, IqFormat, SdrType};

/// Тип SDR устройства (выбор при старте).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceKind {
    /// Встроенный симулятор (не требует железа).
    Simulated,
    /// HackRF One (требует feature `hackrf` + libhackrf).
    HackRf,
    /// ADALM-PlutoSDR (future).
    PlutoSdr,
}

/// Полная конфигурация сессия записи.
#[derive(Debug, Clone)]
pub struct RecorderConfig {
    /// Тип SDR устройство
    pub device: DeviceKind,
    /// Несущая частота (Гц)
    pub center_freq_hz: u64,
    /// Частота дискретизация (Гц)
    pub sample_rate_hz: u32,
    /// Усиление приёмника (дБ)
    pub gain_db: f32,
    /// Формат IQ выборок
    pub iq_format: IqFormat,
    /// Сжатие блоков
    pub compression: Compression,
    /// Путь к выходному .glos файлу
    pub output_path: PathBuf,
    /// Ограничение по времени (None = до Ctrl+C)
    pub duration_secs: Option<u64>,
    /// Выборок в одном IqBlock (влияет на latency и overhead)
    pub block_samples: u32,
    /// Ёмкость кольцевого буфера (chunks; 1 chunk ~ 4096 * sample_size байт)
    pub ring_capacity: usize,
    /// Интервал вывода статистики (секунды)
    pub stats_interval_secs: u64,
}

////////////////////////////////////////////////////////////////////////////////
// Собственные методы
////////////////////////////////////////////////////////////////////////////////

impl RecorderConfig {
    /// Возаращает SdrType для заголовка .glos файла.
    pub fn sdr_type(&self) -> SdrType {
        match self.device {
            DeviceKind::Simulated => SdrType::Unknown,
            DeviceKind::HackRf => SdrType::HackRf,
            DeviceKind::PlutoSdr => SdrType::PlutoSdr,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Общие реализации трейтов для DeviceKind, RecorderConfig
////////////////////////////////////////////////////////////////////////////////

impl std::fmt::Display for DeviceKind {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            DeviceKind::Simulated => write!(f, "sim"),
            DeviceKind::HackRf => write!(f, "hackrf"),
            DeviceKind::PlutoSdr => write!(f, "pluto"),
        }
    }
}

impl std::str::FromStr for DeviceKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sim" | "simulated" => Ok(DeviceKind::Simulated),
            "hackrf" | "hackrf_one" => Ok(DeviceKind::HackRf),
            "pluto" | "plutosdr" | "adalm-pluto" => Ok(DeviceKind::PlutoSdr),
            _ => Err(format!(
                "Unknown device type: '{s}'. Use: sim, hackrf, pluto"
            )),
        }
    }
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            device: DeviceKind::Simulated,
            center_freq_hz: 1_602_000_000,
            sample_rate_hz: 2_000_000,
            gain_db: 40.0,
            iq_format: IqFormat::Int16,
            compression: Compression::None,
            output_path: PathBuf::from("recording.glos"),
            duration_secs: None,
            block_samples: 50_000,
            ring_capacity: 64, // 64 * 4096 * 4 ~ 1 Мб ring buffer
            stats_interval_secs: 5,
        }
    }
}

/// Парсит строку частоты в герцы.
///
/// Поддерживает суффиксы: `GHz`, `MHz`, `kHz`, `Hz` (регистронезависимо).
///
/// # Примеры
/// ```
/// use glos_recorder::config::parse_freq_hz;
/// assert_eq!(parse_freq_hz("1602MHz").unwrap(), 1_602_000_000);
/// assert_eq!(parse_freq_hz("1.602GHz").unwrap(), 1_602_000_000);
/// assert_eq!(parse_freq_hz("2000000").unwrap(), 2_000_000);
/// ```
pub fn parse_freq_hz(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let lower = s.to_lowercase();

    let (num_str, mult) = if let Some(v) = lower.strip_suffix("ghz") {
        (v.trim(), 1_000_000_000_f64)
    } else if let Some(v) = lower.strip_suffix("mhz") {
        (v.trim(), 1_000_000_f64)
    } else if let Some(v) = lower.strip_suffix("khz") {
        (v.trim(), 1_000_f64)
    } else if let Some(v) = lower.strip_suffix("hz") {
        (v.trim(), 1_f64)
    } else {
        // Без суффикса — число в герцах
        return s
            .parse::<u64>()
            .map_err(|e| format!("Invalid frequency '{s}': {e}"));
    };

    let n: f64 = num_str
        .parse()
        .map_err(|e| format!("Invalid frequency value '{num_str}': {e}"))?;

    Ok((n * mult).round() as u64)
}

////////////////////////////////////////////////////////////////////////////////
// Тесты
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_freq_hz() {
        assert_eq!(parse_freq_hz("1602MHz").unwrap(), 1_602_000_000);
        assert_eq!(parse_freq_hz("1.602GHz").unwrap(), 1_602_000_000);
        assert_eq!(parse_freq_hz("2000kHz").unwrap(), 2_000_000);
        assert_eq!(parse_freq_hz("2000000Hz").unwrap(), 2_000_000);
        assert_eq!(parse_freq_hz("2000000").unwrap(), 2_000_000);
        assert!(parse_freq_hz("abc").is_err());
    }

    #[test]
    fn test_device_kind_fromstr() {
        assert_eq!("sim".parse::<DeviceKind>().unwrap(), DeviceKind::Simulated);
        assert_eq!("hackrf".parse::<DeviceKind>().unwrap(), DeviceKind::HackRf);
        assert_eq!("pluto".parse::<DeviceKind>().unwrap(), DeviceKind::PlutoSdr);
        assert!("unknown".parse::<DeviceKind>().is_err());
    }
}

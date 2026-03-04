use glos_types::IqFormat;

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

/// Информация об устройстве (для логирования и заголовка файла).
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub serial: Option<String>,
    pub sample_rate_hz: u32,
    pub center_freq_hz: u64,
    pub gain_db: f32,
    pub sample_format: IqFormat,
}

/// Порция сырых IQ байт, полученная от устройства за один callback/poll.
#[derive(Debug, Clone)]
pub struct IqChunk {
    /// Кол-во IQ пар в `data`
    pub sample_count: u32,
    /// Сырые байты
    pub data: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct HalStats {
    pub chunks_sent: u64,
    pub chunks_dropped: u64,
}

////////////////////////////////////////////////////////////////////////////////
// Общие реализации трейтов для DeviceKind
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

/// Тип SDR устройства
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SdrType {
    /// Hack RF One
    HackRf = 0,
    /// ADALM-PlutoSDR
    PlutoSdr = 1,
    /// USRP B200 family
    UsrpB200 = 2,
    /// Unknown device
    Unknown = 255,
}

impl SdrType {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => SdrType::HackRf,
            1 => SdrType::PlutoSdr,
            2 => SdrType::UsrpB200,
            _ => SdrType::Unknown,
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

use glos_types::IqFormat;

use crate::{DeviceInfo, HalError, HalStats, SdrDevice};

pub struct PlutoDevice {
    // handle, config, etc
}

impl PlutoDevice {
    pub fn new() -> Result<Self, HalError> {
        unimplemented!()
    }
}

impl SdrDevice for PlutoDevice {
    fn info(&self) -> crate::DeviceInfo {
        DeviceInfo {
            name: "PlutoRF".into(),
            serial: None,
            sample_rate_hz: 00_000_000,
            center_freq_hz: 000_000_000,
            gain_db: 0.0,
            sample_format: IqFormat::Int16,
        }
    }

    fn run(
        &mut self,
        _tx: crossbeam_channel::Sender<crate::IqChunk>,
        _stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<HalStats, HalError> {
        unimplemented!()
    }
}

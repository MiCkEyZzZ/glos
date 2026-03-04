use glos_types::IqFormat;

use crate::{DeviceInfo, HalError, HalStats, SdrDevice};

pub struct HackRfDevice {
    // handle, config, etc
}

impl HackRfDevice {
    pub fn new() -> Result<Self, HalError> {
        unimplemented!()
    }
}

impl SdrDevice for HackRfDevice {
    fn info(&self) -> crate::DeviceInfo {
        DeviceInfo {
            name: "HackRF One".to_string(),
            serial: None,
            sample_rate_hz: 20_000_000,
            center_freq_hz: 100_000_000,
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

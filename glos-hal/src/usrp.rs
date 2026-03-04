use std::sync::{atomic::AtomicBool, Arc};

use crossbeam_channel::Sender;
use glos_types::IqFormat;

use crate::{DeviceInfo, HalError, HalStats, IqChunk, SdrDevice};

pub struct UsrpDevice {
    // handle, config, etc
}

impl UsrpDevice {
    pub fn new() -> Result<Self, HalError> {
        unimplemented!()
    }
}

impl SdrDevice for UsrpDevice {
    fn info(&self) -> DeviceInfo {
        DeviceInfo {
            name: "USRP".to_string(),
            serial: None,
            sample_rate_hz: 00_000_000,
            center_freq_hz: 000_000_000,
            gain_db: 0.0,
            sample_format: IqFormat::Int16,
        }
    }

    fn run(
        &mut self,
        _tx: Sender<IqChunk>,
        _stop_flag: Arc<AtomicBool>,
    ) -> Result<HalStats, HalError> {
        unimplemented!()
    }
}

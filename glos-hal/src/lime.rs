use std::sync::{atomic::AtomicBool, Arc};

use crossbeam_channel::Sender;

use crate::{DeviceInfo, HalError, HalStats, IqChunk, SdrDevice};

pub struct LimeSdrDevice {
    // handle, config, etc
}

impl LimeSdrDevice {
    pub fn new() -> Result<Self, HalError> {
        unimplemented!()
    }
}

impl SdrDevice for LimeSdrDevice {
    fn info(&self) -> DeviceInfo {
        unimplemented!()
    }

    fn run(
        &mut self,
        _tx: Sender<IqChunk>,
        _stop_flag: Arc<AtomicBool>,
    ) -> Result<HalStats, HalError> {
        unimplemented!()
    }
}

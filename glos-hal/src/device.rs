use std::sync::{atomic::AtomicBool, Arc};

use crossbeam_channel::Sender;

use crate::{
    types::{DeviceInfo, IqChunk},
    HalError, HalStats,
};

/// Абстракция SDR приёмника.
pub trait SdrDevice: Send {
    /// Информация об устройстве
    fn info(&self) -> DeviceInfo;

    /// Запускает стриминг IQ данных. Блокируется до установки `stop_flag`.
    fn run(
        &mut self,
        tx: Sender<IqChunk>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<HalStats, HalError>;
}

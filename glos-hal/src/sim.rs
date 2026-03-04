use std::{
    f32::consts::PI,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{Sender, TrySendError};
use glos_types::IqFormat;

use crate::{DeviceInfo, HalError, HalStats, IqChunk, SdrDevice};

/// Генерация синтетический IQ сигнал (комплексная синусойда) для тестов.
pub struct SimulatedDevice {
    pub sample_rate_hz: u32,
    pub center_freq_hz: u64,
    pub gain_db: f32,
    pub chunk_samples: u32,
    pub tone_freq_hz: f32,
}

impl SimulatedDevice {
    pub fn new(
        sample_rate_hz: u32,
        center_freq_hz: u64,
        gain_db: f32,
    ) -> Self {
        Self {
            sample_rate_hz,
            center_freq_hz,
            gain_db,
            chunk_samples: 4_096,
            tone_freq_hz: 1_000.0,
        }
    }
}

impl SdrDevice for SimulatedDevice {
    fn info(&self) -> crate::DeviceInfo {
        DeviceInfo {
            name: "Simulate SDR".to_string(),
            serial: Some("SIM-0001".to_string()),
            sample_rate_hz: self.sample_rate_hz,
            center_freq_hz: self.center_freq_hz,
            gain_db: self.gain_db,
            sample_format: IqFormat::Int16,
        }
    }

    fn run(
        &mut self,
        tx: Sender<IqChunk>,
        stop_flag: Arc<AtomicBool>,
    ) -> Result<HalStats, HalError> {
        // период одного сэмпла в нс
        let sample_period_ns = 1_000_000_000f64 / self.sample_rate_hz as f64;
        let mut stats = HalStats::default();

        let start_mono = Instant::now();

        let mut global_sample: u64 = 0;
        let mut _chunks_sent: u64 = 0;

        // Выделяем буфер один раз
        let mut data =
            Vec::<u8>::with_capacity(self.chunk_samples as usize * IqFormat::Int16.sample_size());

        while !stop_flag.load(Ordering::Relaxed) {
            data.clear();

            // Генерация IQ
            for i in 0..self.chunk_samples as u64 {
                let t = (global_sample + i) as f32 / self.sample_rate_hz as f32;

                let i_val = (32_767.0_f32 * (2.0 * PI * self.tone_freq_hz * t).sin()) as i16;
                let q_val = (32_767.0_f32 * (2.0 * PI * self.tone_freq_hz * t).cos()) as i16;

                data.extend_from_slice(&i_val.to_be_bytes());
                data.extend_from_slice(&q_val.to_be_bytes());
            }

            let chunk_data = std::mem::take(&mut data);

            let chunk = IqChunk {
                sample_count: self.chunk_samples,
                data: chunk_data,
            };

            match tx.try_send(chunk) {
                Ok(()) => {
                    stats.chunks_sent += 1;
                }
                Err(TrySendError::Full(_)) => {
                    stats.chunks_dropped += 1;
                } // просто счётчик
                Err(TrySendError::Disconnected(_)) => break,
            }

            global_sample += self.chunk_samples as u64;
            _chunks_sent += 1;

            // pacing — синхронизация по реальному времени
            let expected = Duration::from_nanos((global_sample as f64 * sample_period_ns) as u64);

            let elapsed = start_mono.elapsed();

            if expected > elapsed {
                thread::sleep(expected - elapsed);
            }
        }

        Ok(stats)
    }
}

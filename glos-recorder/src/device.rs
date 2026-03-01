// Код имитирует реальный поток IQ, включая временные метки и rate control, так
// что pipeline видит данные почти как с начстоящего SDR.
// Синусойда просто для тестов, можно проверить запись и обработку сигналов.
// crossbeam_channel используется для асинхронно передачи чанков между потоками.
// stop_flag: Arc<AtomicBool> поток можно остановить безопасно.

use std::{
    f32::consts::PI,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::{Sender, TrySendError};
use glos_types::IqFormat;

use crate::{metrics::RecorderMetrics, DeviceKind, RecorderConfig, RecorderError, RecorderResult};

/// Абстракция SDR приёмника.
// Реализация: [`SimulatedDevice`], и в будущем интерфейс для SDR железки
// `HackRfDevice`, `PlutoSDR`, `Simulated`.
pub trait SdrDevice: Send {
    /// Информация об устройстве
    fn info(&self) -> DeviceInfo;

    /// Запускает стриминг IQ данных. Блокируется до установки `stop_flag`.
    fn run(
        &mut self,
        tx: Sender<IqChunk>,
        metrics: Arc<RecorderMetrics>,
        stop_flag: Arc<AtomicBool>,
    ) -> RecorderResult<()>;
}

/// Порция сырых IQ байт, полученная от устройства за один callback/poll.
#[derive(Debug, Clone)]
pub struct IqChunk {
    /// Unix timestamp начала чанка (наносекунды)
    pub timestamp_ns: u64,
    /// Кол-во IQ пар в `data`
    pub sample_count: u32,
    /// Сырые байты
    pub data: Vec<u8>,
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

/// Генерация синтетический IQ сигнал (комплексная синусойда) для тестов.
pub struct SimulatedDevice {
    pub sample_rate_hz: u32,
    pub center_freq_hz: u64,
    pub gain_db: f32,
    pub chunk_samples: u32,
    pub tone_freq_hz: f32,
}

////////////////////////////////////////////////////////////////////////////////
// Собственные методы
////////////////////////////////////////////////////////////////////////////////

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
    fn info(&self) -> DeviceInfo {
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
        metrics: Arc<RecorderMetrics>,
        stop_flag: Arc<AtomicBool>,
    ) -> RecorderResult<()> {
        // период одного сэмпла в нс
        let sample_period_ns = 1_000_000_000f64 / self.sample_rate_hz as f64;

        let start_mono = Instant::now();
        let start_epoch_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let mut global_sample: u64 = 0;
        let mut _chunks_sent: u64 = 0;

        // Выделяем буфер один раз
        let mut data =
            Vec::<u8>::with_capacity(self.chunk_samples as usize * IqFormat::Int16.sample_size());

        while !stop_flag.load(Ordering::Relaxed) {
            data.clear();

            // timestamp стартового сэмпла в чанке
            let timestamp_ns = start_epoch_ns + (global_sample as f64 * sample_period_ns) as u64;

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
                timestamp_ns,
                sample_count: self.chunk_samples,
                data: chunk_data,
            };

            match tx.try_send(chunk) {
                Ok(()) => {}
                Err(TrySendError::Full(c)) => {
                    metrics
                        .dropped_samples
                        .fetch_add(c.sample_count as u64, Ordering::Relaxed);
                }
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

        Ok(())
    }
}

/// Создаёт нужное устройство по конфигурации.
pub fn create_device(config: &RecorderConfig) -> RecorderResult<Box<dyn SdrDevice>> {
    match &config.device {
        DeviceKind::Simulated => Ok(Box::new(SimulatedDevice::new(
            config.sample_rate_hz,
            config.center_freq_hz,
            config.gain_db,
        ))),
        DeviceKind::HackRf => {
            #[cfg(feature = "hackrf")]
            {
                // TODO: интеграция с hackrfone crate
                // Пример будущей реализации:
                //   let dev = hackrfone::HackRf::open()?;
                //   dev.set_sample_rate(config.sample_rate_hz)?;
                //   dev.set_freq(config.center_freq_hz)?;
                //   dev.set_lna_gain((config.gain_db as u32 / 8) * 8)?;
                //   return Ok(Box::new(HackRfDevice { inner: dev }));
                let _ = config; // подавить неиспользуемое предупреждение
                Err(RecorderError::DeviceNotFound(
                    "HackRF support compiled in but not yet implemented".to_string(),
                ))
            }
            #[cfg(not(feature = "hackrf"))]
            Err(RecorderError::DeviceNotFound(
                "Compiled without HackRF support. \
                 Rebuild with: cargo build --features hackrf"
                    .to_string(),
            ))
        }
        DeviceKind::PlutoSdr => Err(RecorderError::DeviceNotFound(
            "PlutoSDR support not yet implemented (planned for GLOS-3)".to_string(),
        )),
    }
}

////////////////////////////////////////////////////////////////////////////////
// Тесты
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulated_device_info() {
        let dev = SimulatedDevice::new(2_000_000, 1_602_000_000, 40.0);
        let info = dev.info();

        assert_eq!(info.sample_rate_hz, 2_000_000);
        assert_eq!(info.center_freq_hz, 1_602_000_000);
        assert_eq!(info.gain_db, 40.0);
        assert!(info.serial.is_some());
    }

    #[test]
    fn test_simulated_device_generates_chunks() {
        let (tx, rx) = crossbeam_channel::bounded::<IqChunk>(8);
        let metrics = RecorderMetrics::new();
        let stop_flag = Arc::new(AtomicBool::new(false));

        let stop_clone = stop_flag.clone();
        let metrics_clone = metrics.clone();

        // Запускаем устройство в отдельном потоке, останавливаем через 50ms
        let handle = std::thread::spawn(move || {
            let mut dev = SimulatedDevice {
                sample_rate_hz: 2_000_000,
                center_freq_hz: 1_602_000_000,
                gain_db: 40.0,
                chunk_samples: 512, // маленький chunk для быстрого теста
                tone_freq_hz: 1_000.0,
            };
            dev.run(tx, metrics_clone, stop_clone)
        });

        std::thread::sleep(std::time::Duration::from_millis(50));
        stop_flag.store(true, Ordering::Relaxed);

        let _ = handle.join().unwrap();

        // Должны получить хотя бы 1 чанк
        let chunks: Vec<_> = rx.try_iter().collect();
        assert!(!chunks.is_empty(), "ожидаем хотя бы 1 чанк");

        let first = &chunks[0];
        assert_eq!(first.sample_count, 512);
        assert_eq!(first.data.len(), 512 * 4, "Int16: 4 байта/пара");
        assert!(first.timestamp_ns > 0);
    }

    #[test]
    fn test_ring_buffer_drop_on_overflow() {
        // Маленький буфер (1 slot) — сразу переполняется
        let (tx, _rx) = crossbeam_channel::bounded::<IqChunk>(1);
        let metrics = RecorderMetrics::new();
        let stop_flag = Arc::new(AtomicBool::new(false));

        let stop_clone = stop_flag.clone();
        let metrics_clone = metrics.clone();

        let handle = std::thread::spawn(move || {
            let mut dev = SimulatedDevice {
                sample_rate_hz: 2_000_000,
                center_freq_hz: 0,
                gain_db: 0.0,
                chunk_samples: 256,
                tone_freq_hz: 1_000.0,
            };
            dev.run(tx, metrics_clone, stop_clone)
        });

        std::thread::sleep(std::time::Duration::from_millis(30));
        stop_flag.store(true, Ordering::Relaxed);
        let _ = handle.join().unwrap();

        // Должны быть дропы (буфер заполнился после 1 чанка, rx не читает)
        assert!(
            metrics.dropped_samples.load(Ordering::Relaxed) > 0,
            "ожидаем дропы при переполненном буфере"
        );
    }

    #[test]
    fn test_chunk_iq_data_layout() {
        // Проверяем что синусоида корректно закодирована в Big-Endian Int16
        let mut dev = SimulatedDevice {
            sample_rate_hz: 1_000,
            center_freq_hz: 0,
            gain_db: 0.0,
            chunk_samples: 4,
            tone_freq_hz: 250.0, // 250 Гц при 1 kHz → 1/4 периода
        };

        let (tx, rx) = crossbeam_channel::bounded(1);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();
        let metrics = RecorderMetrics::new();

        let handle = std::thread::spawn(move || dev.run(tx, metrics, stop_clone));

        let chunk = rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .unwrap();
        stop_flag.store(true, Ordering::Relaxed);
        let _ = handle.join().unwrap();

        // 4 пары × 4 байта = 16 байт
        assert_eq!(chunk.data.len(), 16);
        // Первая пара: t=0, sin(0)=0, cos(0)=1
        let i0 = i16::from_be_bytes([chunk.data[0], chunk.data[1]]);
        let q0 = i16::from_be_bytes([chunk.data[2], chunk.data[3]]);
        // sin(0) ≈ 0
        assert!(i0.abs() < 100, "I[0] ≈ 0, got {i0}");
        // cos(0) ≈ 32767
        assert!(q0 > 32_000, "Q[0] ≈ 32767, got {q0}");
    }
}

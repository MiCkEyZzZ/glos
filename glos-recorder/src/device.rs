// Код имитирует реальный поток IQ, включая временные метки и rate control, так
// что pipeline видит данные почти как с начстоящего SDR.
// Синусойда просто для тестов, можно проверить запись и обработку сигналов.
// crossbeam_channel используется для асинхронно передачи чанков между потоками.
// stop_flag: Arc<AtomicBool> поток можно остановить безопасно.

use glos_hal::{DeviceKind, SdrDevice, SimulatedDevice};

use crate::{RecorderConfig, RecorderError, RecorderResult};

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
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use glos_hal::{IqChunk, SimulatedDevice};

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
        // Буфер достаточно большой чтобы не переполниться за 50мс при 2 Msps:
        // 2_000_000 / 512 * 0.05 ≈ 195 чанков — берём 256 с запасом.
        let (tx, rx) = crossbeam_channel::bounded::<IqChunk>(256);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        let handle = std::thread::spawn(move || {
            let mut dev = SimulatedDevice {
                sample_rate_hz: 2_000_000,
                center_freq_hz: 1_602_000_000,
                gain_db: 40.0,
                chunk_samples: 512,
                tone_freq_hz: 1_000.0,
            };
            dev.run(tx, stop_clone)
        });

        std::thread::sleep(std::time::Duration::from_millis(50));
        stop_flag.store(true, Ordering::Relaxed);

        let hal_stats = handle.join().unwrap().unwrap();

        let chunks: Vec<_> = rx.try_iter().collect();
        assert!(!chunks.is_empty(), "ожидаем хотя бы 1 чанк");
        assert_eq!(chunks[0].sample_count, 512);
        assert_eq!(chunks[0].data.len(), 512 * 4, "Int16: 4 байта/пара");
        // При буфере 256 дропов быть не должно
        assert_eq!(
            hal_stats.chunks_dropped, 0,
            "chunks_dropped={} — увеличь буфер если тест нестабилен",
            hal_stats.chunks_dropped
        );
    }

    #[test]
    fn test_ring_buffer_drop_on_overflow() {
        let (tx, _rx) = crossbeam_channel::bounded::<IqChunk>(1);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        let handle = std::thread::spawn(move || {
            let mut dev = SimulatedDevice {
                sample_rate_hz: 2_000_000,
                center_freq_hz: 0,
                gain_db: 0.0,
                chunk_samples: 256,
                tone_freq_hz: 1_000.0,
            };
            dev.run(tx, stop_clone) // 2 аргумента
        });

        std::thread::sleep(std::time::Duration::from_millis(30));
        stop_flag.store(true, Ordering::Relaxed);

        let hal_stats = handle.join().unwrap().unwrap();

        // дропы теперь в HalStats, не в RecorderMetrics
        assert!(
            hal_stats.chunks_dropped > 0,
            "ожидаем дропы при переполненном буфере, got {}",
            hal_stats.chunks_dropped
        );
    }

    #[test]
    fn test_chunk_iq_data_layout() {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_flag.clone();

        let handle = std::thread::spawn(move || {
            let mut dev = SimulatedDevice {
                sample_rate_hz: 1_000,
                center_freq_hz: 0,
                gain_db: 0.0,
                chunk_samples: 4,
                tone_freq_hz: 250.0, // 250 Гц при 1 kHz → 1/4 периода
            };
            dev.run(tx, stop_clone) // 2 аргумента
        });

        let chunk = rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .unwrap();
        stop_flag.store(true, Ordering::Relaxed);
        let _ = handle.join().unwrap();

        assert_eq!(chunk.data.len(), 16); // 4 пары × 4 байта

        let i0 = i16::from_be_bytes([chunk.data[0], chunk.data[1]]);
        let q0 = i16::from_be_bytes([chunk.data[2], chunk.data[3]]);

        assert!(i0.abs() < 100, "I[0] ≈ 0, got {i0}");
        assert!(q0 > 32_000, "Q[0] ≈ 32767, got {q0}");
    }
}

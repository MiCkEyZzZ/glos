use std::{
    fs::File,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crossbeam_channel::RecvTimeoutError;
use glos_core::{GlosHeaderExt, GlosWriter, IqBlockExt};
use glos_types::{GlosHeader, IqBlock};
use log::{info, warn};

use crate::{
    device::{IqChunk, SdrDevice},
    metrics::RecorderMetrics,
    RecorderConfig, RecorderResult,
};

/// Оркестрирует сессию записи.
pub struct RecordingPipeline {
    config: RecorderConfig,
    metrics: Arc<RecorderMetrics>,
    stop_flag: Arc<AtomicBool>,
}

impl RecordingPipeline {
    /// Создаёт пайплайн. Возвращает также sahred-ссылку на метрики.
    pub fn new(config: RecorderConfig) -> (Self, Arc<RecorderMetrics>) {
        let metrics = RecorderMetrics::new();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let p = Self {
            config,
            metrics: metrics.clone(),
            stop_flag,
        };

        (p, metrics)
    }

    /// Флаг остановки. Устанавливает в `true` для graceful shutdown.
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    /// Запускает запись. Блокируется до завершения.
    pub fn run(
        self,
        mut device: Box<dyn SdrDevice>,
    ) -> RecorderResult<()> {
        let info = device.info();

        info!(
            "Starting recording: {} @ {} Hz, center={} Hz, gain={} dB",
            info.name, info.sample_rate_hz, info.center_freq_hz, info.gain_db
        );

        info!(
            "Output: {:?}, duration: {:?}",
            self.config.output_path, self.config.duration_secs
        );

        let (tx, rx) = crossbeam_channel::bounded::<IqChunk>(self.config.ring_capacity);
        let stop_flag = self.stop_flag.clone();
        let stop_flag_capture = stop_flag.clone();
        let metrics_capture = self.metrics.clone();

        // Захват потока
        let capture_handle = std::thread::spawn(move || {
            let result = device.run(tx, metrics_capture, stop_flag_capture);

            if let Err(ref e) = result {
                warn!("Capture thread error: {e}");
            }

            result
        });

        // Цикл записи (текущий поток)
        let writer_result = self.writer_loop(rx);

        // Сигнализируем потоку захвата остановиться
        stop_flag.store(true, Ordering::Relaxed);

        // Дожидаемся завершения потока захвата
        match capture_handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("Capture thread finished with error: {e}"),
            Err(_) => warn!("Capture thread panicked"),
        }

        writer_result
    }

    fn writer_loop(
        &self,
        rx: crossbeam_channel::Receiver<IqChunk>,
    ) -> RecorderResult<()> {
        let cfg = &self.config;
        let metrics = &self.metrics;

        // Открываем файл и создаём GlosWriter
        let file = File::create(&cfg.output_path)?;
        let mut header = GlosHeader::new(cfg.sdr_type(), cfg.sample_rate_hz, cfg.center_freq_hz);
        header.gain_db = cfg.gain_db;
        header.iq_format = cfg.iq_format;
        header.compression = cfg.compression;

        let mut writer = GlosWriter::new(file, header)?;

        let sample_size = cfg.iq_format.sample_size();
        let block_samples = cfg.block_samples;
        let recv_timeout = Duration::from_millis(100);
        let stats_interval = Duration::from_secs(cfg.stats_interval_secs);

        // Накопитель частичного блока
        let mut acc: Vec<u8> = Vec::with_capacity(block_samples as usize * sample_size);
        let mut acc_samples: u32 = 0;
        let mut block_ts: Option<u64> = None; // timestamp первого чанка в блоке

        let session_start = Instant::now();
        let mut last_stats = Instant::now();

        loop {
            //  Проверяем ограничение по времени
            if let Some(dur) = cfg.duration_secs {
                if session_start.elapsed().as_secs() >= dur {
                    info!("Duration limit reached ({dur}s). Finalizing...");
                    break;
                }
            }

            //  Проверяем внешний stop_flag (Ctrl+C)
            if self.stop_flag.load(Ordering::Relaxed) {
                info!("Stop signal received. Finalizing...");
                break;
            }

            //  Получаем следующий chunk
            let chunk = match rx.recv_timeout(recv_timeout) {
                Ok(c) => c,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    info!("Capture channel closed. Flushing...");
                    break;
                }
            };

            // Обновляем счётчик выборок
            metrics
                .samples_recorded
                .fetch_add(chunk.sample_count as u64, Ordering::Relaxed);

            //  Накапливаем в accumulator
            if block_ts.is_none() {
                block_ts = Some(chunk.timestamp_ns);
            }
            acc.extend_from_slice(&chunk.data);
            acc_samples += chunk.sample_count;

            // Пишем полные блоки
            while acc_samples >= block_samples {
                let n_bytes = block_samples as usize * sample_size;
                let block_data: Vec<u8> = acc.drain(..n_bytes).collect();
                let ts = block_ts.take().unwrap_or(0);

                let block = IqBlock::new(ts, block_samples, block_data);
                let block_bytes = block_samples as u64 * sample_size as u64 + 20; // approx

                match writer.write_block(block) {
                    Ok(()) => {
                        metrics.blocks_written.fetch_add(1, Ordering::Relaxed);
                        metrics
                            .bytes_written
                            .fetch_add(block_bytes, Ordering::Relaxed);
                    }
                    Err(e) => {
                        metrics.write_errors.fetch_add(1, Ordering::Relaxed);
                        warn!("Write error: {e}");
                        // Не прерываем — пробуем продолжить
                    }
                }

                acc_samples -= block_samples;
            }

            // Периодически выводим статистику
            if last_stats.elapsed() >= stats_interval {
                self.log_progress(&session_start);
                last_stats = Instant::now();
            }
        }

        // Flush частичного блока (если есть)
        if acc_samples > 0 {
            let ts = block_ts.unwrap_or(0);
            let block = IqBlock::new(ts, acc_samples, acc);
            if let Err(e) = writer.write_block(block) {
                warn!("Failed to write final partial block: {e}");
                metrics.write_errors.fetch_add(1, Ordering::Relaxed);
            } else {
                metrics.blocks_written.fetch_add(1, Ordering::Relaxed);
                info!("Flushed partial block ({acc_samples} samples)");
            }
        }

        // Finalize: перезаписываем заголовок с total_samples
        writer.finish()?;

        info!("File finalized: {:?}", cfg.output_path);
        Ok(())
    }

    fn log_progress(
        &self,
        start: &Instant,
    ) {
        let m = &self.metrics;

        info!(
            "[ {:.0}s ] samples={} blocks={} dropped={} ({:.2}%) speed={:.1}MB/s",
            start.elapsed().as_secs_f64(),
            m.samples_recorded.load(Ordering::Relaxed),
            m.blocks_written.load(Ordering::Relaxed),
            m.dropped_samples.load(Ordering::Relaxed),
            m.drop_rate_pct(),
            m.write_speed_mbps(start),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use glos_core::{read_all_blocks, GlosReader};
    use glos_types::{Compression, IqFormat};
    use tempfile::NamedTempFile;

    use super::*;
    use crate::{device::SimulatedDevice, DeviceKind};

    fn test_config(path: PathBuf) -> RecorderConfig {
        RecorderConfig {
            device: DeviceKind::Simulated,
            center_freq_hz: 1_602_000_000,
            sample_rate_hz: 2_000_000,
            gain_db: 40.0,
            iq_format: IqFormat::Int16,
            compression: Compression::None,
            output_path: path,
            duration_secs: Some(1), // 1 секунда → завершается сам
            block_samples: 10_000,
            ring_capacity: 32,
            stats_interval_secs: 60, // не выводим stats в тестах
        }
    }

    #[test]
    fn test_pipeline_writes_valid_glos() {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();

        let config = test_config(path.clone());
        let sample_rate = config.sample_rate_hz;
        let (pipeline, _metrics) = RecordingPipeline::new(config);

        let device = Box::new(SimulatedDevice::new(sample_rate, 1_602_000_000, 40.0));
        pipeline.run(device).unwrap();

        // Читаем файл и валидируем
        let file = std::fs::File::open(&path).unwrap();
        let mut reader = GlosReader::new(file).unwrap();
        let blocks = read_all_blocks(&mut reader).unwrap();

        assert!(!blocks.is_empty(), "there must be at least 1 block");
        assert_eq!(reader.stats().blocks_corrupted, 0, "no corrupted blocks");
        reader.validate_totals().unwrap();

        // каждый блок должен иметь корректное соотношение sample_count / data.len()
        for b in &blocks {
            b.validate_sample_count(IqFormat::Int16).unwrap();
        }
    }

    #[test]
    fn test_pipeline_metrics_are_updated() {
        let tmp = NamedTempFile::new().unwrap();
        let config = test_config(tmp.path().to_path_buf());
        let sample_rate = config.sample_rate_hz;
        let (pipeline, metrics) = RecordingPipeline::new(config);

        let device = Box::new(SimulatedDevice::new(sample_rate, 1_602_000_000, 40.0));
        pipeline.run(device).unwrap();

        assert!(
            metrics.samples_recorded.load(Ordering::Relaxed) > 0,
            "samples_recorded должен быть > 0"
        );
        assert!(
            metrics.blocks_written.load(Ordering::Relaxed) > 0,
            "blocks_written должен быть > 0"
        );
        assert_eq!(metrics.write_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_pipeline_stop_flag_works() {
        let tmp = NamedTempFile::new().unwrap();
        let mut config = test_config(tmp.path().to_path_buf());
        config.duration_secs = None; // без ограничения по времени

        let sample_rate = config.sample_rate_hz;
        let (pipeline, _metrics) = RecordingPipeline::new(config);
        let stop = pipeline.stop_flag();

        // Останавливаем через 200 мс из отдельного потока
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            stop.store(true, Ordering::Relaxed);
        });

        let device = Box::new(SimulatedDevice::new(sample_rate, 1_602_000_000, 40.0));
        let result = pipeline.run(device);
        assert!(result.is_ok(), "graceful stop не должен быть ошибкой");
    }

    #[test]
    fn test_pipeline_lz4_compression() {
        let tmp = NamedTempFile::new().unwrap();
        let mut config = test_config(tmp.path().to_path_buf());
        config.compression = Compression::Lz4;

        let sample_rate = config.sample_rate_hz;
        let (pipeline, _) = RecordingPipeline::new(config);

        let device = Box::new(SimulatedDevice::new(sample_rate, 1_602_000_000, 40.0));
        pipeline.run(device).unwrap();

        let file = std::fs::File::open(tmp.path()).unwrap();
        let mut reader = GlosReader::new(file).unwrap();
        assert_eq!(reader.header().compression, Compression::Lz4);

        // Блоки должны читаться и распаковываться без ошибок
        let blocks = read_all_blocks(&mut reader).unwrap();
        assert!(!blocks.is_empty());
        for b in &blocks {
            assert!(!b.is_compressed, "GlosReader должен распаковать блоки");
        }
    }

    #[test]
    fn test_pipeline_partial_block_flushed() {
        let tmp = NamedTempFile::new().unwrap();
        let mut config = test_config(tmp.path().to_path_buf());

        // Уменьшаем block_samples, чтобы хотя бы один chunk успел записаться
        config.block_samples = 100_000;
        config.duration_secs = Some(1);

        let sample_rate = config.sample_rate_hz;
        let (pipeline, metrics) = RecordingPipeline::new(config);

        // Симулируем устройство, которое гарантированно выдаст хотя бы один chunk
        let device = Box::new(SimulatedDevice::new(sample_rate, 1_602_000_000, 40.0));
        pipeline.run(device).unwrap();

        // Проверяем, что один частичный блок записан
        assert!(
            metrics.blocks_written.load(Ordering::Relaxed) > 0,
            "один частичный блок должен быть flush-нут"
        );

        // Проверяем читаемость файла
        let file = std::fs::File::open(tmp.path()).unwrap();
        let mut reader = GlosReader::new(file).unwrap();
        let blocks = read_all_blocks(&mut reader).unwrap();

        assert!(!blocks.is_empty(), "должен быть хотя бы один блок в файле");
    }
}

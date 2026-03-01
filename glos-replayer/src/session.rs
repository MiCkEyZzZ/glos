use std::{
    fs::File,
    net::UdpSocket,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use glos_core::{GlosReader, ReadStats, ReplayMetrics, TimingController, UdpPacket};
use glos_types::GlosHeader;

use crate::{ReplayConfiq, ReplayError, ReplayResult};

/// Сессия воспроизведения (single-threaded).
pub struct ReplaySession {
    config: ReplayConfiq,
    metrics: Arc<ReplayMetrics>,
    stop_flag: Arc<AtomicBool>,
    pause_flag: Arc<AtomicBool>,
}

impl ReplaySession {
    /// Создаёт сессию, проверяя конфигурацию.
    pub fn new(config: ReplayConfiq) -> ReplayResult<Self> {
        if config.speed <= 0.0 {
            return Err(ReplayError::Config("speed must be > 0".to_string()));
        }

        Ok(Self {
            config,
            metrics: ReplayMetrics::new(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        self.stop_flag.clone()
    }

    pub fn pause_flag(&self) -> Arc<AtomicBool> {
        self.pause_flag.clone()
    }

    pub fn metrics(&self) -> Arc<ReplayMetrics> {
        self.metrics.clone()
    }

    /// Запускает воспроизведение. Блокирует до EOF или stop_flag.
    pub fn run(self) -> ReplayResult<()> {
        let cfg = &self.config;
        let metrics = &self.metrics;
        let stop = &self.stop_flag;
        let session_start = Instant::now();
        let stats_interval = std::time::Duration::from_secs(cfg.stats_interval_secs);

        // Создаём UDP-сокет
        let socket = UdpSocket::bind(&cfg.bind_addr)?;
        socket.connect(&cfg.target_addr)?;

        // Читаем заголовок один раз и выводим инфо
        let header = {
            let f = File::open(&cfg.input_path)?;
            let r = GlosReader::new(f)?;
            r.header().clone()
        };

        Self::print_header_info(&header, cfg);

        let mut timing = TimingController::new(cfg.speed, self.pause_flag.clone());
        let mut last_stats = Instant::now();
        let mut loop_count = 0u64;

        'outer: loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            loop_count += 1;

            if loop_count > 1 {
                eprintln!("[replayer] Loop #[loop_count]");

                timing.reset();
            }

            let file = File::open(&cfg.input_path)?;
            let mut reader = GlosReader::new(file)?;

            while let Some(result) = reader.next_block() {
                if stop.load(Ordering::Relaxed) {
                    break 'outer;
                }

                let block = match result {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("[replayer] Skipping corrupted block: {e}");
                        continue;
                    }
                };

                // Speed-controlled timing
                timing.wait_for(block.timestamp_ns, metrics);

                // Сереализуем в UDP-пакет
                let payload = match UdpPacket::encode(&block) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("[replayer] Encode error (block too large): {e}");
                        metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };

                // Отправляем
                match socket.send(&payload) {
                    Ok(n) => {
                        metrics.packets_sent.fetch_add(1, Ordering::Relaxed);
                        metrics
                            .samples_sent
                            .fetch_add(block.sample_count as u64, Ordering::Relaxed);
                        metrics.bytes_sent.fetch_add(n as u64, Ordering::Relaxed);
                    }
                    Err(e) => {
                        eprintln!("[replayer] UDP send error: {e}");
                        metrics.send_errors.fetch_add(1, Ordering::Relaxed);
                    }
                }

                // Переодически выводим прогресс
                if last_stats.elapsed() >= stats_interval {
                    Self::log_progress(metrics, &session_start, reader.stats());
                    last_stats = Instant::now();
                }
            }

            // Graceful EOF
            eprintln!(
                "[replayer] EOF: {} blocks, {} samples",
                reader.stats().blocks_ok,
                reader.stats().samples_recovered,
            );

            if !cfg.loop_playback {
                break;
            }
        }

        metrics.print_summary(&session_start);

        Ok(())
    }

    fn print_header_info(
        h: &GlosHeader,
        cfg: &ReplayConfiq,
    ) {
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("  Input         : {:?}", cfg.input_path);
        eprintln!("  Target        : {}", cfg.target_addr);
        eprintln!("  Speed         : {}x", cfg.speed);
        eprintln!("  Loop          : {}", cfg.loop_playback);
        eprintln!("  SDR type      : {:?}", h.sdr_type);
        eprintln!("  Sample rate   : {:.3} MHz", h.sample_rate as f64 / 1e6);
        eprintln!("  Center freq   : {:.3} MHz", h.center_freq as f64 / 1e6);
        eprintln!("  IQ format     : {:?}", h.iq_format);
        eprintln!("  Total samples : {}", h.total_samples);
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    fn log_progress(
        m: &ReplayMetrics,
        start: &Instant,
        stats: &ReadStats,
    ) {
        eprintln!(
            "[ {:.0}s ] pkts={} sampled={} underruns={} errors={} timing_err={:.1}µs blocks_ok={}",
            start.elapsed().as_secs_f64(),
            m.packets_sent.load(Ordering::Relaxed),
            m.samples_sent.load(Ordering::Relaxed),
            m.underruns.load(Ordering::Relaxed),
            m.send_errors.load(Ordering::Relaxed),
            m.avg_timing_error_us(),
            stats.blocks_ok,
        );
    }
}

/// Парсит `udp://host:port` или просто `host:port`.
pub fn parse_udp_target(s: &str) -> Result<String, String> {
    let addr = s.strip_prefix("udp://").unwrap_or(s);
    addr.parse::<std::net::SocketAddr>()
        .map(|a| a.to_string())
        .map_err(|e| format!("Invalid UDP address '{s}': {e}"))
}

#[cfg(test)]
mod tests {
    use glos_core::{GlosHeaderExt, GlosWriter, IqBlockExt};
    use glos_types::{Compression, IqBlock, IqFormat, SdrType};
    use tempfile::NamedTempFile;

    use super::*;

    /// Создаёт временный .glos файд с `n_blocks` блоками по `samples` выборок.
    fn make_glos_file(
        n_blocks: u64,
        samples: u32,
    ) -> NamedTempFile {
        let tmp = NamedTempFile::new().unwrap();
        let mut header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);

        header.iq_format = IqFormat::Int16;
        header.compression = Compression::None;

        let file = std::fs::File::create(tmp.path()).unwrap();
        let mut writer = GlosWriter::new(file, header).unwrap();

        // Период выборки = 500нс при 2 Msps
        let period_ns: u64 = 1_000_000_000 / 2_000_000;

        for i in 0..n_blocks {
            let ts_ns = 1_704_067_200_000_000_000u64 + i * samples as u64 * period_ns;
            let data = vec![0u8; samples as usize * 4];

            writer
                .write_block(IqBlock::new(ts_ns, samples, data))
                .unwrap();
        }

        writer.finish().unwrap();

        tmp
    }

    #[test]
    fn test_replay_sends_udp_packets() {
        // Поднимаем UDP-слушатель на свободном порту
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        listener
            .set_read_timeout(Some(std::time::Duration::from_millis(500)))
            .unwrap();

        let tmp = make_glos_file(3, 100);

        let config = ReplayConfiq {
            input_path: tmp.path().to_path_buf(),
            target_addr: addr.clone(),
            speed: 100.0, // очень быстро, без задержек
            loop_playback: false,
            stats_interval_secs: 60,
            bind_addr: "0.0.0.0:0".to_string(),
        };

        let session = ReplaySession::new(config).unwrap();

        session.run().unwrap();

        // Читаем все пакеты
        let mut received = 0usize;
        let mut buf = vec![0u8; 65536];

        while let Ok(n) = listener.recv(&mut buf) {
            let (ts, count, data) = UdpPacket::decode(&buf[..n]).unwrap();

            assert!(ts > 0, "timestamp must be > 0");
            assert_eq!(count, 100);
            assert_eq!(data.len(), 400); // 100 × 4 байта Int16
            received += 1;
        }

        assert_eq!(received, 3, "expecting 3 packets (one per block)");
    }

    #[test]
    fn test_replay_metrics_updated() {
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        let tmp = make_glos_file(5, 50);
        let config = ReplayConfiq {
            input_path: tmp.path().to_path_buf(),
            target_addr: addr,
            speed: 100.0,
            loop_playback: false,
            stats_interval_secs: 60,
            bind_addr: "0.0.0.0:0".to_string(),
        };

        let session = ReplaySession::new(config).unwrap();
        let metrics = session.metrics();
        session.run().unwrap();

        assert_eq!(metrics.packets_sent.load(Ordering::Relaxed), 5);
        assert_eq!(metrics.samples_sent.load(Ordering::Relaxed), 250);
        assert_eq!(metrics.send_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_replay_stop_flag() {
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();

        // Большой файл — много блоков
        let tmp = make_glos_file(100, 1000);
        let config = ReplayConfiq {
            input_path: tmp.path().to_path_buf(),
            target_addr: addr,
            speed: 100.0,
            loop_playback: false,
            stats_interval_secs: 60,
            bind_addr: "0.0.0.0:0".to_string(),
        };

        let session = ReplaySession::new(config).unwrap();
        let stop = session.stop_flag();
        let metrics = session.metrics();

        // Останавливаем после первых нескольких пакетов
        let stop_clone = stop.clone();
        let m_clone = metrics.clone();
        std::thread::spawn(move || {
            // Ждём пока придёт хотя бы 2 пакета
            while m_clone.packets_sent.load(Ordering::Relaxed) < 2 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            stop_clone.store(true, Ordering::Relaxed);
        });

        session.run().unwrap();

        // Остановились раньше конца файла (< 100 пакетов)
        assert!(
            metrics.packets_sent.load(Ordering::Relaxed) < 100,
            "stop_flag должен прервать воспроизведение до конца файла"
        );
    }

    #[test]
    fn test_replay_lz4_file() {
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        listener
            .set_read_timeout(Some(std::time::Duration::from_millis(500)))
            .unwrap();

        // Создаём сжатый файл
        let tmp = NamedTempFile::new().unwrap();
        let mut header = GlosHeader::new(SdrType::HackRf, 2_000_000, 1_602_000_000);
        header.iq_format = IqFormat::Int16;
        header.compression = Compression::Lz4;

        let file = std::fs::File::create(tmp.path()).unwrap();
        let mut writer = GlosWriter::new(file, header).unwrap();
        let data = vec![42u8; 200]; // 50 Int16 samples
        writer
            .write_block(IqBlock::new(1_000_000_000, 50, data))
            .unwrap();
        writer.finish().unwrap();

        let config = ReplayConfiq {
            input_path: tmp.path().to_path_buf(),
            target_addr: addr,
            speed: 100.0,
            loop_playback: false,
            stats_interval_secs: 60,
            bind_addr: "0.0.0.0:0".to_string(),
        };

        let session = ReplaySession::new(config).unwrap();
        session.run().unwrap();

        // Проверяем что пакет пришёл и данные корректны
        let mut buf = vec![0u8; 65536];
        let n = listener.recv(&mut buf).unwrap();
        let (_ts, count, data) = UdpPacket::decode(&buf[..n]).unwrap();
        assert_eq!(count, 50);
        assert_eq!(data, vec![42u8; 200]);
    }

    #[test]
    fn test_parse_udp_target() {
        assert_eq!(
            parse_udp_target("udp://127.0.0.1:5555").unwrap(),
            "127.0.0.1:5555"
        );
        assert_eq!(
            parse_udp_target("127.0.0.1:5555").unwrap(),
            "127.0.0.1:5555"
        );
        assert!(parse_udp_target("not_an_addr").is_err());
    }

    #[test]
    fn test_replay_config_invalid_speed() {
        let config = ReplayConfiq {
            speed: -1.0,
            ..Default::default()
        };
        assert!(ReplaySession::new(config).is_err());

        let config = ReplayConfiq {
            speed: 0.0,
            ..Default::default()
        };
        assert!(ReplaySession::new(config).is_err());
    }
}

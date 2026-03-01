use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use crate::IqBlock;

/// Максимальный размер UDP payload (стандартный IPv4).
pub const UDP_MAX_PAYLOAD: usize = 65_507;

pub const UDP_TIMESTAMP_SIZE: usize = 8;
pub const UDP_SAMPLE_COUNT_SIZE: usize = 2;

/// Размер заголовка UDP-пакета GLOS.
pub const UDP_HEADER_SIZE: usize = UDP_TIMESTAMP_SIZE + UDP_SAMPLE_COUNT_SIZE;

/// UDP-пакет с IQ-данными.
///
/// Формат передачи данных (big-endian):
/// ```text
/// [0..8]  TIMESTAMP       u64  — метка времени блока (наносекунды)
/// [8..10] SAMPLE_COUNT    u16  — количество IQ пар
/// [10..]  IQ_DATA         [u8] — сырые IQ байты
/// ```
pub struct UdpPacket;

/// Lock-free метрики сессии воспроизведения.
#[derive(Debug, Default)]
pub struct ReplayMetrics {
    pub packets_sent: AtomicU64,
    pub samples_sent: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub underruns: AtomicU64,
    pub send_errors: AtomicU64,
    pub timing_error_ns_total: AtomicU64,
}

/// Управляет темпом воспроизведения с учётом `speed` и компенсаций дрейфа.
///
/// Принцип: для каждого блока вычисляем когда он должен быть отправлен
/// относительно старта сессии, учитывая `speed`. Если мы опережаем - спим. Если
/// отстаём - фиксируем underrun и продолжаем без задержки.
pub struct TimingController {
    pub speed: f64,
    session_start: Instant,
    file_start_ns: Option<u64>,
    paused: Arc<AtomicBool>,
}

impl UdpPacket {
    /// Сериализует блок в UDP payload.
    pub fn encode(block: &IqBlock) -> Result<Vec<u8>, String> {
        let max_data = UDP_MAX_PAYLOAD - UDP_HEADER_SIZE;

        if block.data.len() > max_data {
            return Err(format!(
                "Block data {} bytes exceeds UDP payload lomit {} bytes",
                block.data.len(),
                max_data,
            ));
        }

        if block.sample_count > u16::MAX as u32 {
            return Err(format!(
                "sample_count {} exceeds u16 range ({})",
                block.sample_count,
                u16::MAX
            ));
        }

        let sample_count = block.sample_count as u16;
        let mut buf = Vec::with_capacity(UDP_HEADER_SIZE + block.data.len());

        buf.extend_from_slice(&block.timestamp_ns.to_be_bytes());
        buf.extend_from_slice(&sample_count.to_be_bytes());
        buf.extend_from_slice(&block.data);

        Ok(buf)
    }

    /// Десериализует UDP payload в `(timestamp_ns, sample_count, iq_data)`.
    pub fn decode(buf: &[u8]) -> Result<(u64, u16, &[u8]), String> {
        if buf.len() < UDP_HEADER_SIZE {
            return Err(format!(
                "Packet too short: {} < {}",
                buf.len(),
                UDP_HEADER_SIZE,
            ));
        }

        let timestamp_ns = u64::from_be_bytes(buf[0..8].try_into().unwrap());
        let sample_count = u16::from_be_bytes(buf[8..10].try_into().unwrap());
        let iq_data = &buf[UDP_HEADER_SIZE..];

        Ok((timestamp_ns, sample_count, iq_data))
    }
}

impl ReplayMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Возвращает среднюю скорость отправки (Msps).
    pub fn throughput_msps(
        &self,
        start: &Instant,
    ) -> f64 {
        let secs = start.elapsed().as_secs_f64().max(1e-9);

        self.samples_sent.load(Ordering::Relaxed) as f64 / secs / 1_000_000.0
    }

    /// Возвращает среднюю ошибку тайминга (мкс).
    pub fn avg_timing_error_us(&self) -> f64 {
        let pkts = self.packets_sent.load(Ordering::Relaxed);

        if pkts == 0 {
            return 0.0;
        }

        self.timing_error_ns_total.load(Ordering::Relaxed) as f64 / pkts as f64 / 1_000.0
    }

    pub fn print_summary(
        &self,
        start: &Instant,
    ) {
        let elapsed = start.elapsed().as_secs_f64();

        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        eprintln!("  Duration      : {elapsed:.1}s");
        eprintln!(
            "  Packets sent  : {}",
            self.packets_sent.load(Ordering::Relaxed)
        );
        eprintln!(
            "  Samples sent  : {}",
            self.samples_sent.load(Ordering::Relaxed)
        );
        eprintln!(
            "  Bytes sent    : {:.1} MB",
            self.bytes_sent.load(Ordering::Relaxed) as f64 / 1e6
        );
        eprintln!(
            "  Underruns     : {}",
            self.underruns.load(Ordering::Relaxed)
        );
        eprintln!(
            "  Send errors   : {}",
            self.send_errors.load(Ordering::Relaxed)
        );
        eprintln!("  Throughput    : {:.3} Msps", self.throughput_msps(start));
        eprintln!("  Timing error  : {:.1} µs avg", self.avg_timing_error_us());
        eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }
}

impl TimingController {
    pub fn new(
        speed: f64,
        paused: Arc<AtomicBool>,
    ) -> Self {
        Self {
            speed: speed.max(0.01),
            session_start: Instant::now(),
            file_start_ns: None,
            paused,
        }
    }

    /// Сбрасывает таймер (вызывается при старте / resume после длинной паузы).
    pub fn reset(&mut self) {
        self.session_start = Instant::now();
        self.file_start_ns = None;
    }

    /// Ждёт нужного момента для отправки блока с `timestamp_ns`.
    ///
    /// Возвращает фактическую ошибку тайминга (нс) для метрик.
    pub fn wait_for(
        &mut self,
        timestamp_ns: u64,
        metrics: &ReplayMetrics,
    ) -> u64 {
        // Ждём пока пауза активна
        while self.paused.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(20));
            // Сдвигаем session_start на время паузы чтобы не получить burst
            self.session_start = Instant::now()
                .checked_sub(self.elapsed_virtual_ns())
                .unwrap_or_else(Instant::now);
        }

        // Инициализируем привязку файлового времени к реальному
        let file_start = *self.file_start_ns.get_or_insert(timestamp_ns);

        // Сколько виртуального времени файла прошло от начала
        let file_offset_ns = timestamp_ns.saturating_sub(file_start);

        // Сколько реального времени это займёт при текущем speed
        let real_offset_ns = (file_offset_ns as f64 / self.speed) as u64;

        // Сколько реального времени прошло с начала сессии
        let elapsed_ns = self.session_start.elapsed().as_nanos() as u64;

        if real_offset_ns > elapsed_ns {
            let sleep_ns = real_offset_ns - elapsed_ns;
            std::thread::sleep(Duration::from_nanos(sleep_ns));

            // Ошибка тайминга после сна
            let actual_elapsed = self.session_start.elapsed().as_nanos() as u64;
            let error = actual_elapsed.saturating_sub(real_offset_ns);
            metrics
                .timing_error_ns_total
                .fetch_add(error, Ordering::Relaxed);
            error
        } else {
            // Отстаём — underrun
            let lag = elapsed_ns - real_offset_ns;
            if lag > 1_000_000 {
                // > 1ms отставания — считаем underrun
                metrics.underruns.fetch_add(1, Ordering::Relaxed);
            }
            metrics
                .timing_error_ns_total
                .fetch_add(lag, Ordering::Relaxed);
            lag
        }
    }

    pub fn elapsed_virtual_ns(&self) -> Duration {
        Duration::from_nanos((self.session_start.elapsed().as_nanos() as f64 * self.speed) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_udp_packet_encode_decode_roundtrip() {
        let block = IqBlock::new(1_704_067_200_000_000_000, 100, vec![42u8; 400]);
        let encoded = UdpPacket::encode(&block).unwrap();

        assert_eq!(encoded.len(), UDP_HEADER_SIZE + 400);

        let (ts, count, data) = UdpPacket::decode(&encoded).unwrap();

        assert_eq!(ts, 1_704_067_200_000_000_000);
        assert_eq!(count, 100);
        assert_eq!(data, vec![42u8; 400]);
    }

    #[test]
    fn test_udp_packet_header_big_endian() {
        let block = IqBlock::new(0x0102030405060708, 0x0A0B, vec![0u8; 4]);
        let encoded = UdpPacket::encode(&block).unwrap();

        // timestamp BE
        assert_eq!(
            &encoded[0..8],
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );

        // sample_count BE (0x0A0B)
        assert_eq!(&encoded[8..10], &[0x0A, 0x0B]);
    }

    #[test]
    fn test_udp_packet_too_large() {
        let block = IqBlock::new(0, 1, vec![0u8; UDP_MAX_PAYLOAD]);

        assert!(UdpPacket::encode(&block).is_err());
    }

    #[test]
    fn test_udp_decode_too_short() {
        assert!(UdpPacket::decode(&[0u8; 5]).is_err());
    }

    #[test]
    fn test_replay_metrics_throughput() {
        let m = ReplayMetrics::new();

        m.samples_sent.store(2_000_000, Ordering::Relaxed);

        let start = Instant::now() - Duration::from_secs(2);
        let tp = m.throughput_msps(&start);

        assert!((tp - 1.0).abs() < 0.1, "expected ~1 Msps, got {tp}");
    }

    #[test]
    fn test_timing_controller_speed_1x() {
        let paused = Arc::new(AtomicBool::new(false));
        let mut ctrl = TimingController::new(1.0, paused);
        let metrics = ReplayMetrics::new();

        // Первый блок — устанавливает file_start
        ctrl.wait_for(1_000_000_000, &metrics);

        // Второй блок через 50мс файлового времени
        let before = Instant::now();
        ctrl.wait_for(1_000_000_000 + 50_000_000, &metrics);
        let elapsed = before.elapsed();

        // Должны подождать ~50мс +/- 20мс
        assert!(
            elapsed.as_millis() >= 30,
            "Expected pause ~50ms, got {}ms",
            elapsed.as_millis()
        );
        assert!(
            elapsed.as_millis() <= 150,
            "Pause too long: {}ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_timing_controller_speed_2x() {
        let paused = Arc::new(AtomicBool::new(false));
        let mut ctrl = TimingController::new(2.0, paused);
        let metrics = ReplayMetrics::new();

        ctrl.wait_for(0, &metrics);

        let before = Instant::now();

        // 100мс файлового времени при 2х ->  ~ 50мс реального
        ctrl.wait_for(100_000_000, &metrics);

        let elapsed = before.elapsed();

        assert!(
            elapsed.as_millis() <= 100,
            "at 2x the pause should be ≤100ms, got {}ms",
            elapsed.as_millis()
        );
    }

    #[test]
    fn test_timing_controller_pause_resume() {
        let paused = Arc::new(AtomicBool::new(false));
        let mut ctrl = TimingController::new(10.0, paused.clone());
        let metrics = ReplayMetrics::new();

        ctrl.wait_for(0, &metrics);

        // Ставим паузу
        paused.store(true, Ordering::Relaxed);

        // Снимаем паузу через 50мс в отдельном потоке
        let p = paused.clone();

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));

            p.store(false, Ordering::Relaxed);
        });

        // wait_for должен ждать пока пауза не снята
        let before = Instant::now();

        ctrl.wait_for(10_000_000, &metrics); // 10мс файлового при 10х - 1мс реального

        let elapsed = before.elapsed();

        // Должен дождаться снятия паузы (>= 50мс)
        assert!(
            elapsed.as_millis() >= 40,
            "pause: waiting ≥40ms, received {}ms",
            elapsed.as_millis()
        );
    }
}

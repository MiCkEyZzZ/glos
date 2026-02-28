use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};

/// Метрики, обновляемые lock-free из нескольких потоков.
#[derive(Debug, Default)]
pub struct RecorderMetrics {
    pub samples_recorded: AtomicU64,
    pub blocks_written: AtomicU64,
    pub dropped_samples: AtomicU64,
    pub write_errors: AtomicU64,
    pub bytes_written: AtomicU64,
}

/// Snapshot мутрики для отображения / тестирования.
#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub duration_secs: f64,
    pub samples_recorded: u64,
    pub blocks_written: u64,
    pub dropped_samples: u64,
    pub write_errors: u64,
    pub bytes_written: u64,
    pub throughput_msps: f64,
    pub write_speed_mbps: f64,
    pub drop_rate_pct: f64,
}

impl RecorderMetrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn throughput_msps(
        &self,
        elapsed: &Instant,
    ) -> f64 {
        let secs = elapsed.elapsed().as_secs_f64();

        if secs < 1e-9 {
            return 0.0;
        }

        self.samples_recorded.load(Ordering::Relaxed) as f64 / secs / 1_000_000.0
    }

    /// Скорость записи в МБ/с.
    pub fn write_speed_mbps(
        &self,
        elapsed: &Instant,
    ) -> f64 {
        let secs = elapsed.elapsed().as_secs_f64();

        if secs < 1e-9 {
            return 0.0;
        }

        self.bytes_written.load(Ordering::Relaxed) as f64 / secs / 1_000_000.0
    }

    /// Процент потерянных выборок (0.0-100.0).
    pub fn drop_rate_pct(&self) -> f64 {
        let recorded = self.samples_recorded.load(Ordering::Relaxed);
        let dropped = self.dropped_samples.load(Ordering::Relaxed);
        let total = recorded + dropped;

        if total == 0 {
            0.0
        } else {
            dropped as f64 / total as f64 * 100.0
        }
    }

    /// Итоговая сводка для вывода в конце сессии.
    pub fn summary(
        &self,
        elapsed: &Instant,
    ) -> MetricsSummary {
        MetricsSummary {
            duration_secs: elapsed.elapsed().as_secs_f64(),
            samples_recorded: self.samples_recorded.load(Ordering::Relaxed),
            blocks_written: self.blocks_written.load(Ordering::Relaxed),
            dropped_samples: self.dropped_samples.load(Ordering::Relaxed),
            write_errors: self.write_errors.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            throughput_msps: self.throughput_msps(elapsed),
            write_speed_mbps: self.write_speed_mbps(elapsed),
            drop_rate_pct: self.drop_rate_pct(),
        }
    }
}

impl std::fmt::Display for MetricsSummary {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        writeln!(f, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")?;
        writeln!(f, "  Duration      : {:.1}s", self.duration_secs)?;
        writeln!(f, "  Samples       : {}", self.samples_recorded)?;
        writeln!(f, "  Blocks        : {}", self.blocks_written)?;
        writeln!(
            f,
            "  Dropped       : {} ({:.2}%)",
            self.dropped_samples, self.drop_rate_pct
        )?;
        writeln!(f, "  Write errors  : {}", self.write_errors)?;
        writeln!(
            f,
            "  Bytes written : {:.1} MB",
            self.bytes_written as f64 / 1e6
        )?;
        writeln!(f, "  Throughput    : {:.3} Msps", self.throughput_msps)?;
        writeln!(f, "  Write speed   : {:.1} MB/s", self.write_speed_mbps)?;
        write!(f, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn test_initial_metrics_zero() {
        let metrics = RecorderMetrics::new();
        let start = Instant::now();
        let summary = metrics.summary(&start);

        assert_eq!(summary.samples_recorded, 0);
        assert_eq!(summary.blocks_written, 0);
        assert_eq!(summary.dropped_samples, 0);
        assert_eq!(summary.write_errors, 0);
        assert_eq!(summary.bytes_written, 0);
        assert_eq!(summary.throughput_msps, 0.0);
        assert_eq!(summary.write_speed_mbps, 0.0);
        assert_eq!(summary.drop_rate_pct, 0.0);
    }

    #[test]
    fn test_drop_rate_calculation() {
        let metrics = RecorderMetrics::new();

        metrics.samples_recorded.store(80, Ordering::Relaxed);
        metrics.dropped_samples.store(20, Ordering::Relaxed);

        let drop_rate = metrics.drop_rate_pct();

        assert!((drop_rate - 20.0).abs() < 1e-6);
    }

    #[test]
    fn test_throughput_and_write_speed() {
        let metrics = RecorderMetrics::new();

        metrics.samples_recorded.store(2_000_000, Ordering::Relaxed);
        metrics.bytes_written.store(10_000_000, Ordering::Relaxed);

        let start = Instant::now() - Duration::from_secs(2);
        let summary = metrics.summary(&start);

        // throughput: 2_000_000 / 2 / 1_000_000 = 1.0 Msps
        // write_speed: 10_000_000 bytes / 2s / 1_000_000 ≈ 5 MB/s
        assert!((summary.throughput_msps - 1.0).abs() < 0.01);
        assert!((summary.write_speed_mbps - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_summary_snapshot_consistency() {
        let metrics = RecorderMetrics::new();
        metrics.samples_recorded.store(100, Ordering::Relaxed);
        metrics.blocks_written.store(10, Ordering::Relaxed);
        metrics.dropped_samples.store(5, Ordering::Relaxed);
        metrics.write_errors.store(1, Ordering::Relaxed);
        metrics.bytes_written.store(1_000_000, Ordering::Relaxed);

        let start = Instant::now() - Duration::from_secs(1);
        let summary = metrics.summary(&start);

        assert_eq!(summary.samples_recorded, 100);
        assert_eq!(summary.blocks_written, 10);
        assert_eq!(summary.dropped_samples, 5);
        assert_eq!(summary.write_errors, 1);
        assert_eq!(summary.bytes_written, 1_000_000);
        assert!(summary.throughput_msps > 0.0);
        assert!(summary.write_speed_mbps > 0.0);
        assert!(summary.drop_rate_pct > 0.0);
    }

    #[test]
    fn test_multithreaded_updates() {
        let metrics = RecorderMetrics::new();
        let metrics_arc = metrics.clone();

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let m = metrics_arc.clone();
                thread::spawn(move || {
                    for _ in 0..1_000 {
                        m.samples_recorded.fetch_add(1, Ordering::Relaxed);
                        m.bytes_written.fetch_add(1_024, Ordering::Relaxed);
                        m.dropped_samples.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(metrics.samples_recorded.load(Ordering::Relaxed), 4_000);
        assert_eq!(metrics.dropped_samples.load(Ordering::Relaxed), 4_000);
        assert_eq!(metrics.bytes_written.load(Ordering::Relaxed), 4_096_000);
    }
}

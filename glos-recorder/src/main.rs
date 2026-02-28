use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use clap::Parser;
use glos_core::{Compression, IqFormat};
use glos_recorder::{create_device, parse_freq_hz, DeviceKind, RecorderConfig, RecordingPipeline};
use log::{error, info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "glos-recorder",
    version = env!("CARGO_PKG_VERSION"),
    about = "Record IQ samples from SDR device to .glos file",
    long_about = None,
)]
struct Cli {
    /// SDR устройство: sim, hackrf, pluto
    #[arg(short, long, default_value = "sim")]
    device: String,
    /// Несущая частота (1602MHz, 1.602GHz, 1602000000)
    #[arg(short = 'f', long, default_value = "1602MHz")]
    freq: String,
    /// Частота дискретизации (2MHz, 2000000)
    #[arg(short = 'r', long, default_value = "2MHz")]
    rate: String,
    /// Усиление приёмника, дБ
    #[arg(short, long, default_value = "40.0")]
    gain: f32,
    /// Путь к выходному файлу
    #[arg(short, long, default_value = "recording.glos")]
    output: PathBuf,
    /// Ограничение записи (секунды). По умолчанию: до Ctrl+C
    #[arg(short, long)]
    duration: Option<u64>,
    /// Формат IQ выборок: int8, int16, float32
    #[arg(long, default_value = "int16")]
    format: String,
    /// Сжатие: none, lz4
    #[arg(long, default_value = "none")]
    compress: String,
    /// Выборок в блоке (влияет на latency/overhead)
    #[arg(long, default_value = "50000")]
    block_samples: u32,
    /// Ёмкость кольцевого буфера (кол-во chunk-слотов, 1 chunk ≈ 16 KB)
    #[arg(long, default_value = "256")]
    ring_capacity: usize,
    /// Интервал вывода статистики (секунды)
    #[arg(long, default_value = "5")]
    stats_interval: u64,
    /// Тихий режим (только ошибки)
    #[arg(short, long)]
    quiet: bool,
}

fn parse_iq_format(s: &str) -> Result<IqFormat, String> {
    match s.to_lowercase().as_str() {
        "int8" | "i8" => Ok(IqFormat::Int8),
        "int16" | "i16" => Ok(IqFormat::Int16),
        "float32" | "f32" => Ok(IqFormat::Float32),
        _ => Err(format!(
            "Unknown IQ format '{s}'. Use: int8, int16, float32"
        )),
    }
}

fn parse_compression(s: &str) -> Result<Compression, String> {
    match s.to_lowercase().as_str() {
        "none" | "no" | "off" => Ok(Compression::None),
        "lz4" => Ok(Compression::Lz4),
        _ => Err(format!("Unknown compression '{s}'. Use: none, lz4")),
    }
}

fn main() {
    let cli = Cli::parse();
    let level = if cli.quiet { "error" } else { "info" };

    env_logger::Builder::new()
        .filter_level(level.parse().unwrap())
        .format_target(false)
        .format_timestamp_secs()
        .init();

    let device_kind: DeviceKind = match cli.device.parse() {
        Ok(d) => d,
        Err(e) => {
            error!("{e}");
            std::process::exit(1);
        }
    };

    let center_freq_hz = match parse_freq_hz(&cli.freq) {
        Ok(f) => f,
        Err(e) => {
            error!("--freq: {e}");
            std::process::exit(1);
        }
    };

    let sample_rate_hz = match parse_freq_hz(&cli.rate) {
        Ok(r) if r <= u32::MAX as u64 => r as u32,
        Ok(r) => {
            error!("--rate {r} Hz exceeds u32::MAX");
            std::process::exit(1);
        }
        Err(e) => {
            error!("--rate: {e}");
            std::process::exit(1);
        }
    };

    let iq_format = match parse_iq_format(&cli.format) {
        Ok(f) => f,
        Err(e) => {
            error!("--format: {e}");
            std::process::exit(1);
        }
    };

    let compression = match parse_compression(&cli.compress) {
        Ok(c) => c,
        Err(e) => {
            error!("--compress: {e}");
            std::process::exit(1);
        }
    };

    let config = RecorderConfig {
        device: device_kind,
        center_freq_hz,
        sample_rate_hz,
        gain_db: cli.gain,
        iq_format,
        compression,
        output_path: cli.output.clone(),
        duration_secs: cli.duration,
        block_samples: cli.block_samples,
        ring_capacity: cli.ring_capacity,
        stats_interval_secs: cli.stats_interval,
    };

    let device = match create_device(&config) {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to open device: {e}");
            std::process::exit(1);
        }
    };

    let (pipeline, metrics) = RecordingPipeline::new(config);
    let stop_flag: Arc<AtomicBool> = pipeline.stop_flag();

    let stop_ctrlc = stop_flag.clone();

    if let Err(e) = ctrlc::set_handler(move || {
        if stop_ctrlc.swap(true, Ordering::SeqCst) {
            // Второй Ctrl+C — принудительный выход
            warn!("Force exit");
            std::process::exit(130);
        }
        warn!("Ctrl+C received — finishing current block and finalizing file...");
    }) {
        warn!("Failed to set Ctrl+C handler: {e}");
    }

    // Выводим конфигурацию
    let sample_size = iq_format.sample_size();
    let data_rate_mbs = sample_rate_hz as f64 * sample_size as f64 / 1_000_000.0;

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("  Device        : {}", cli.device);
    info!("  Center freq   : {:.3} MHz", center_freq_hz as f64 / 1e6);
    info!("  Sample rate   : {:.3} Msps", sample_rate_hz as f64 / 1e6);
    info!("  IQ format     : {:?} ({sample_size} B/sample)", iq_format);
    info!("  Compression   : {:?}", compression);
    info!("  Data rate     : {:.1} MB/s", data_rate_mbs);
    info!("  Output        : {:?}", cli.output);

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let session_start = Instant::now();

    match pipeline.run(device) {
        Ok(()) => {}
        Err(e) => {
            error!("Recording failed: {e}");
            std::process::exit(1);
        }
    }

    // --- Итоговая статистика ---
    let summary = metrics.summary(&session_start);
    info!("\n{summary}");

    if metrics.dropped_samples.load(Ordering::Relaxed) > 0 {
        warn!(
            "⚠ {} samples dropped ({:.2}% loss). Consider: larger --ring-capacity or lower --rate",
            metrics.dropped_samples.load(Ordering::Relaxed),
            summary.drop_rate_pct
        );
    }

    if metrics.write_errors.load(Ordering::Relaxed) > 0 {
        warn!(
            "⚠ {} write errors occurred. Check disk space and I/O.",
            metrics.write_errors.load(Ordering::Relaxed)
        );
        std::process::exit(1);
    }

    info!("✓ Recording complete: {:?}", cli.output);
}

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

use clap::Parser;
use glos_replayer::{parse_udp_target, ReplayConfiq, ReplaySession};
use log::{error, info, warn};
use signal_hook::{consts::signal::SIGTSTP, flag};

#[derive(Parser, Debug)]
#[command(
    name = "glos-replayer",
    version = env!("CARGO_PKG_VERSION"),
    about = "Replay .glos IQ recordings over UDP",
    long_about = None,
)]
struct Cli {
    /// Входной .glos файл
    #[arg(short, long)]
    input: PathBuf,

    /// UDP адрес назначения (udp://host:port или host:port)
    #[arg(short, long, default_value = "udp://127.0.0.1:5555")]
    output: String,

    /// Коэффициент скорости: 0.5, 1.0, 2.0 и т.д.
    #[arg(short, long, default_value = "1.0")]
    speed: f64,

    /// Повторять файл бесконечно
    #[arg(long)]
    r#loop: bool,

    /// Интервал вывода статистики (секунды)
    #[arg(long, default_value = "5")]
    stats_interval: u64,

    /// UDP bind-адрес (обычно не нужно менять)
    #[arg(long, default_value = "0.0.0.0:0")]
    bind: String,

    /// Тихий режим (только ошибки)
    #[arg(short, long)]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();

    let level = if cli.quiet { "error" } else { "info" };

    env_logger::Builder::new()
        .filter_level(level.parse().unwrap())
        .format_target(false)
        .format_timestamp_secs()
        .init();

    // ⚠️ Правовое предупреждение
    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    warn!("  ⚠ LEGAL NOTICE: glos-replayer operates in loopback/offline mode.");
    warn!("  Retransmitting RF signals (incl. GNSS) without a license is");
    warn!("  ILLEGAL in most jurisdictions. Do NOT connect to RF hardware.");
    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Парсинг UDP-адреса
    let target_addr = match parse_udp_target(&cli.output) {
        Ok(a) => a,
        Err(e) => {
            error!("--output: {e}");
            std::process::exit(1);
        }
    };

    if !cli.input.exists() {
        error!("Input file not found: {:?}", cli.input);
        std::process::exit(1);
    }

    if cli.speed <= 0.0 {
        error!("--speed must be > 0");
        std::process::exit(1);
    }

    let config = ReplayConfiq {
        input_path: cli.input.clone(),
        target_addr,
        speed: cli.speed,
        loop_playback: cli.r#loop,
        stats_interval_secs: cli.stats_interval,
        bind_addr: cli.bind.clone(),
    };

    let session = match ReplaySession::new(config) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to create session: {e}");
            std::process::exit(1);
        }
    };

    let stop_flag: Arc<AtomicBool> = session.stop_flag();
    let pause_flag: Arc<AtomicBool> = session.pause_flag();
    let metrics = session.metrics();

    // Ctrl+C → graceful shutdown
    let stop_ctrlc = stop_flag.clone();
    let mut ctrlc_count = 0u8;

    ctrlc::set_handler(move || {
        ctrlc_count += 1;

        if ctrlc_count == 1 {
            warn!("Ctrl+C — stopping after current block...");
            stop_ctrlc.store(true, Ordering::Relaxed);
        } else {
            warn!("Force exit");
            std::process::exit(130);
        }
    })
    .unwrap_or_else(|e| warn!("Failed to set Ctrl+C handler: {e}"));

    // SIGTSTP (Ctrl+Z) → pause toggle
    if let Err(e) = flag::register(SIGTSTP, pause_flag.clone()) {
        warn!("Failed to register SIGTSTP handler: {e}");
    }

    info!(
        "Starting replay: {:?} → {} @ {}x{}",
        cli.input,
        cli.output,
        cli.speed,
        if cli.r#loop { " (loop)" } else { "" }
    );

    let session_start = Instant::now();

    match session.run() {
        Ok(()) => {}
        Err(e) => {
            error!("Replay failed: {e}");
            std::process::exit(1);
        }
    }

    let elapsed = session_start.elapsed().as_secs_f64();
    info!("✓ Replay complete in {elapsed:.1}s");

    info!(
        "  Packets: {}  Samples: {}  Underruns: {}  Errors: {}",
        metrics.packets_sent.load(Ordering::Relaxed),
        metrics.samples_sent.load(Ordering::Relaxed),
        metrics.underruns.load(Ordering::Relaxed),
        metrics.send_errors.load(Ordering::Relaxed),
    );

    if metrics.send_errors.load(Ordering::Relaxed) > 0 {
        warn!("⚠ Some packets failed to send — check UDP target and network.");
        std::process::exit(1);
    }
}

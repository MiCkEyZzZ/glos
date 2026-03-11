use std::{fs, io::Write, path::PathBuf, time::Instant};

use clap::Parser;
use glos_analyzer::{
    decode_iq, export_spectrum_csv, export_spectrum_png, export_waterfall_csv,
    export_waterfall_png, render_ascii_spectrum, render_ascii_waterfall, PeakDetector,
    SpectrumConfig, SpectrumProcessor, WaterfallBuffer, WindowFunction,
};
use glos_core::GlosReader;
use log::{error, info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "glos-analizer",
    version = env!("CARGO_PKG_VERSION"),
    about = "Analize IQ spectrum from .glos recordings",
    long_about = None,
)]
struct Cli {
    #[arg(short, long)]
    input: PathBuf,
    #[arg(long, default_value = "1024")]
    fft_size: usize,
    #[arg(long, default_value = "hann")]
    window: String,
    #[arg(long, default_value = "8")]
    avg: usize,
    #[arg(long, default_value = "50")]
    waterfall_rows: usize,
    #[arg(long, default_value = "1")]
    display_every: usize,
    #[arg(long, default_value = "80")]
    ascii_width: usize,
    #[arg(long, default_value = "16")]
    ascii_height: usize,
    #[arg(long, default_value = "10.0")]
    peak_snr: f32,
    #[arg(long)]
    export_png: Option<PathBuf>,
    #[arg(long)]
    export_csv: Option<PathBuf>,
    #[arg(long)]
    waterfall_png: Option<PathBuf>,
    #[arg(long)]
    waterfall_csv: Option<PathBuf>,
    #[arg(long)]
    no_display: bool,
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

    // Проверка входного файла
    if !cli.input.exists() {
        error!("Input file not found: {:?}", cli.input);
        std::process::exit(1);
    }

    // Парсинг параметров
    let window: WindowFunction = match cli.window.parse() {
        Ok(w) => w,
        Err(e) => {
            error!("--window: {e}");
            std::process::exit(1);
        }
    };

    if !cli.fft_size.is_power_of_two() || cli.fft_size < 64 {
        error!(
            "--fft-size must be a power of 2 and >= 64 (got {})",
            cli.fft_size
        );
        std::process::exit(1);
    }

    // --- Читаем заголовок файла ---
    let file = match fs::File::open(&cli.input) {
        Ok(f) => f,
        Err(e) => {
            error!("Cannot open {:?}: {e}", cli.input);
            std::process::exit(1);
        }
    };

    let mut reader = match GlosReader::new(file) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to read GLOS header: {e}");
            std::process::exit(1);
        }
    };

    let header = reader.header().clone();

    let config = SpectrumConfig {
        fft_size: cli.fft_size,
        window,
        avg_count: cli.avg,
        waterfall_rows: cli.waterfall_rows,
        sample_rate_hz: header.sample_rate,
        center_freq_hz: header.center_freq,
    };

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("  File          : {:?}", cli.input);
    info!("  SDR type      : {:?}", header.sdr_type);
    info!(
        "  Sample rate   : {:.3} MHz",
        header.sample_rate as f64 / 1e6
    );
    info!(
        "  Center freq   : {:.3} MHz",
        header.center_freq as f64 / 1e6
    );
    info!("  IQ format     : {:?}", header.iq_format);
    info!("  Total samples : {}", header.total_samples);
    info!("  FFT size      : {}", cli.fft_size);
    info!("  Window        : {}", window);
    info!("  Avg count     : {}", cli.avg);
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // --- Обработка блоков ---
    let mut proc = SpectrumProcessor::new(config.clone());
    let mut wf = WaterfallBuffer::new(cli.waterfall_rows, cli.fft_size);
    let detector = PeakDetector::new(cli.peak_snr, 5);

    let start = Instant::now();
    let mut spectra_count = 0usize;
    let mut blocks_total = 0usize;
    let mut last_spectrum = None;
    let mut last_metrics = None;

    // Продолжаем читать через reader (уже частично прочитан для заголовка)
    while let Some(result) = reader.next_block() {
        let block = match result {
            Ok(b) => b,
            Err(e) => {
                warn!("Skipping corrupted block: {e}");
                continue;
            }
        };

        blocks_total += 1;

        // Декодируем IQ
        let samples = decode_iq(&block.data, header.iq_format);

        // FFT + скользящее среднее
        if let Some(spectrum) = proc.process_block(&samples, block.timestamp_ns) {
            let metrics = detector.analyze(&spectrum, config.sample_rate_hz, config.center_freq_hz);

            // Добавляем строку в waterfall
            wf.push(&spectrum.power_db);

            spectra_count += 1;

            // ASCII вывод
            if !cli.no_display && spectra_count.is_multiple_of(cli.display_every) {
                let rendered = render_ascii_spectrum(
                    &spectrum,
                    &metrics,
                    &config,
                    cli.ascii_width,
                    cli.ascii_height,
                );
                // Очищаем строки перед перерисовкой
                if spectra_count > cli.display_every {
                    let lines = rendered.lines().count() + 2;
                    print!("\x1B[{}A", lines); // cursor up
                }
                print!("{rendered}");
                std::io::stdout().flush().ok();
            }

            last_spectrum = Some(spectrum);
            last_metrics = Some(metrics);
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let read_stats = reader.stats();

    println!();
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("  Blocks read    : {blocks_total}");
    info!("  Spectra        : {spectra_count}");
    info!("  Corrupted      : {}", read_stats.blocks_corrupted);
    info!("  Elapsed        : {elapsed:.2}s");

    if let Some(ref m) = last_metrics {
        info!("  Noise floor    : {:.1} dBFS", m.noise_floor_db);
        info!("  Peak freq      : {:.3} MHz", m.peak_freq_hz / 1e6);
        info!("  Peak SNR       : {:.1} dB", m.peak_snr_db);
        info!("  Peaks found    : {}", m.peaks.len());
    }
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // --- Waterfall ASCII (финальный) ---
    if !cli.no_display && wf.filled_rows() > 0 {
        println!("{}", render_ascii_waterfall(&wf, cli.ascii_width));
    }

    // --- Экспорт ---
    let mut export_ok = true;

    if let (Some(path), Some(spectrum)) = (&cli.export_csv, &last_spectrum) {
        let csv = export_spectrum_csv(spectrum, &config);
        match fs::write(path, csv) {
            Ok(()) => info!("✓ Spectrum CSV: {path:?}"),
            Err(e) => {
                error!("CSV export failed: {e}");
                export_ok = false;
            }
        }
    }

    if let (Some(path), Some(spectrum)) = (&cli.export_png, &last_spectrum) {
        if let Some(metrics) = &last_metrics {
            match export_spectrum_png(spectrum, metrics, &config, 1200, 400) {
                Ok(bytes) => match fs::write(path, bytes) {
                    Ok(()) => info!("✓ Spectrum PNG: {path:?}"),
                    Err(e) => {
                        error!("PNG write failed: {e}");
                        export_ok = false;
                    }
                },
                Err(e) => {
                    error!("PNG export failed: {e}");
                    export_ok = false;
                }
            }
        }
    }

    if let Some(path) = &cli.waterfall_csv {
        if wf.filled_rows() > 0 {
            let csv = export_waterfall_csv(&wf);
            match fs::write(path, csv) {
                Ok(()) => info!("✓ Waterfall CSV: {path:?}"),
                Err(e) => {
                    error!("Waterfall CSV failed: {e}");
                    export_ok = false;
                }
            }
        }
    }

    if let Some(path) = &cli.waterfall_png {
        if wf.filled_rows() > 0 {
            match export_waterfall_png(&wf, 1200, cli.waterfall_rows as u32 * 4) {
                Ok(bytes) => match fs::write(path, bytes) {
                    Ok(()) => info!("✓ Waterfall PNG: {path:?}"),
                    Err(e) => {
                        error!("Waterfall PNG write failed: {e}");
                        export_ok = false;
                    }
                },
                Err(e) => {
                    error!("Waterfall PNG failed: {e}");
                    export_ok = false;
                }
            }
        } else {
            warn!("Waterfall is empty — not enough data for waterfall PNG");
        }
    }

    if !export_ok {
        std::process::exit(1);
    }
}

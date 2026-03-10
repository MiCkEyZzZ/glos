use std::{
    cmp::Ordering,
    f32::{self, consts::PI},
    sync::Arc,
};

use glos_types::IqFormat;
use image::{ImageBuffer, ImageEncoder, Rgb};
use rustfft::{num_complex::Complex32, Fft, FftPlanner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowFunction {
    Rectangular,
    Hann,
    Blackman,
}

/// Конфигурация анализатора спектра.
#[derive(Debug, Clone)]
pub struct SpectrumConfig {
    pub fft_size: usize,
    pub window: WindowFunction,
    pub avg_count: usize,
    pub waterfall_rows: usize,
    pub sample_rate_hz: u32,
    pub center_freq_hz: u64,
}

/// Спектр мощности: bins * Дб.
#[derive(Debug, Clone)]
pub struct PowerSpectrum {
    pub power_db: Vec<f32>,
    pub timestamp_ns: u64,
}

/// Основной процессор FFT + sliding average.
pub struct SpectrumProcessor {
    config: SpectrumConfig,
    fft: Arc<dyn Fft<f32>>,
    window_coeff: Vec<f32>,
    power_norm: f32,
    avg_acc: Vec<f32>,
    avg_filled: usize,
}

/// Кольцевой буфер истории спектра (2D: rows * fft_bins).
pub struct WaterfallBuffer {
    rows: usize,
    cols: usize,
    data: Vec<Vec<f32>>,
    head: usize,
    filled: usize,
}

/// Обнаруженный пик в спектре.
#[derive(Debug, Clone)]
pub struct Peak {
    pub bin_idx: usize,
    pub freq_hz: f64,
    pub power_db: f32,
    pub snr_db: f32,
}

/// Метрики спектра.
#[derive(Debug, Clone)]
pub struct SpectrumMetrics {
    pub noise_floor_db: f32,
    pub peaks: Vec<Peak>,
    pub peak_snr_db: f32,
    pub peak_freq_hz: f64,
}

/// Анализирует спектр: шумовой пол, пики, SNR.
pub struct PeakDetector {
    pub min_snr_db: f32,
    pub min_bin_distance: usize,
}

////////////////////////////////////////////////////////////////////////////////
// Собственные методы
////////////////////////////////////////////////////////////////////////////////

impl WindowFunction {
    /// Генерирует коэффициенты оконной ф-ии длиной `n`.
    pub fn coefficients(
        self,
        n: usize,
    ) -> Vec<f32> {
        match self {
            WindowFunction::Rectangular => vec![1.0f32; n],
            WindowFunction::Hann => (0..n)
                .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (n - 1) as f32).cos()))
                .collect(),
            WindowFunction::Blackman => (0..n)
                .map(|i| {
                    let x = 2.0 * PI * i as f32 / (n - 1) as f32;
                    0.42 - 0.5 * x.cos() + 0.08 * (2.0 * x).cos()
                })
                .collect(),
        }
    }

    /// Мощность нормировочный коэффициент (sum of squared coefficients).
    pub fn power_norm(
        self,
        n: usize,
    ) -> f32 {
        let c = self.coefficients(n);

        c.iter().map(|x| x * x).sum::<f32>()
    }
}

impl PowerSpectrum {
    /// Центральные частоты бинов (Гц) относительно center_freq.
    pub fn bin_frequencies(
        &self,
        sample_rate_hz: u32,
        center_freq_hz: u64,
    ) -> Vec<f64> {
        let n = self.power_db.len();
        let bin_width = sample_rate_hz as f64 / n as f64;

        (0..n)
            .map(|i| {
                let shifted = i as i64 - n as i64 / 2;

                center_freq_hz as f64 + shifted as f64 * bin_width
            })
            .collect()
    }
}

impl SpectrumProcessor {
    pub fn new(config: SpectrumConfig) -> Self {
        let window_coeff = config.window.coefficients(config.fft_size);
        let power_norm = config.window.power_norm(config.fft_size);
        let avg_acc = vec![0.0f32; config.fft_size];
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(config.fft_size);

        Self {
            config,
            fft,
            window_coeff,
            power_norm,
            avg_acc,
            avg_filled: 0,
        }
    }

    pub fn process_block(
        &mut self,
        samples: &[Complex32],
        timestamp_ns: u64,
    ) -> Option<PowerSpectrum> {
        let n = self.config.fft_size;

        if samples.len() < n {
            return None;
        }

        let mut last_spectrum: Option<Vec<f32>> = None;
        let step = (n / 2).max(1);

        // Обрабатываем все поля окна в блоке
        for window_start in (0..=samples.len() - n).step_by(step) {
            let window_samples = &samples[window_start..window_start + n];

            let mut buf: Vec<Complex32> = window_samples
                .iter()
                .zip(self.window_coeff.iter())
                .map(|(s, &w)| Complex32::new(s.re * w, s.im * w))
                .collect();

            self.fft.process(&mut buf);

            // fftshift: вторая половина идёт в начало
            let mut shifted = Vec::with_capacity(n);

            shifted.extend_from_slice(&buf[n / 2..]);
            shifted.extend_from_slice(&buf[..n / 2]);

            // Мощность в дБFS
            let power: Vec<f32> = shifted
                .iter()
                .map(|c| {
                    let mag_sq = c.norm_sqr() / self.power_norm;
                    10.0 * (mag_sq.max(1e-12)).log10()
                })
                .collect();

            // Накапливаем для скользящего среднего
            for (acc, &p) in self.avg_acc.iter_mut().zip(power.iter()) {
                *acc += p;
            }

            self.avg_filled += 1;

            if self.avg_filled >= self.config.avg_count {
                let avg: Vec<f32> = self
                    .avg_acc
                    .iter()
                    .map(|&s| s / self.avg_filled as f32)
                    .collect();

                self.avg_acc.fill(0.0);
                self.avg_filled = 0;

                last_spectrum = Some(avg);
            }
        }

        last_spectrum.map(|power_db| PowerSpectrum {
            power_db,
            timestamp_ns,
        })
    }

    pub fn reset(&mut self) {
        self.avg_acc.fill(0.0);
        self.avg_filled = 0;
    }

    pub fn config(&self) -> &SpectrumConfig {
        &self.config
    }
}

impl WaterfallBuffer {
    pub fn new(
        rows: usize,
        cols: usize,
    ) -> Self {
        Self {
            rows,
            cols,
            data: vec![vec![f32::NEG_INFINITY; cols]; rows],
            head: 0,
            filled: 0,
        }
    }

    /// Добавляет новую строку спектра.
    pub fn push(
        &mut self,
        spectrum: &[f32],
    ) {
        let n = self.cols.min(spectrum.len());

        self.data[self.head][..n].copy_from_slice(&spectrum[..n]);
        self.head = (self.head + 1) % self.rows;

        if self.filled < self.rows {
            self.filled += 1;
        }
    }

    /// Вовзращает все строки в хронологическом порядке (старые первые)
    pub fn rows_ordered(&self) -> Vec<&[f32]> {
        let mut result = Vec::with_capacity(self.filled);
        let start = if self.filled < self.rows {
            0
        } else {
            self.head
        };

        for i in 0..self.filled {
            let idx = (start + i) % self.rows;
            result.push(self.data[idx].as_slice());
        }

        result
    }

    pub fn filled_rows(&self) -> usize {
        self.filled
    }

    pub fn cols(&self) -> usize {
        self.cols
    }
}

impl PeakDetector {
    pub fn new(
        min_snr_db: f32,
        min_bin_distance: usize,
    ) -> Self {
        Self {
            min_snr_db,
            min_bin_distance,
        }
    }

    /// Вычисляет метрики для заданного спектра мощности.
    pub fn analyze(
        &self,
        spectrum: &PowerSpectrum,
        sample_rate_hz: u32,
        center_freq_hz: u64,
    ) -> SpectrumMetrics {
        let power = &spectrum.power_db;
        let n = power.len();

        // Шумовой пол медиана
        let noise_floor_db = median(power);

        // Поиск локальных максимумов выше порога
        let threshold = noise_floor_db + self.min_snr_db;
        let mut candidates: Vec<(usize, f32)> = Vec::new();

        for i in 1..power.len().saturating_sub(1) {
            if power[i] > threshold && power[i] >= power[i - 1] && power[i] >= power[i + 1] {
                candidates.push((i, power[i]));
            }
        }

        // Сортируем по убыванию
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        // Подавление немаксимальных значений
        let mut peaks: Vec<Peak> = Vec::new();
        let bin_width = sample_rate_hz as f64 / n as f64;

        'outer: for (idx, pwr) in &candidates {
            for exsting in &peaks {
                if idx.abs_diff(exsting.bin_idx) < self.min_bin_distance {
                    continue 'outer;
                }
            }

            let shifted = *idx as i64 - n as i64 / 2;
            let freq_hz = center_freq_hz as f64 + shifted as f64 * bin_width;
            let snr_db = pwr - noise_floor_db;

            peaks.push(Peak {
                bin_idx: *idx,
                freq_hz,
                power_db: *pwr,
                snr_db,
            });
        }

        let (peak_snr_db, peak_freq_hz) = peaks
            .first()
            .map(|p| (p.snr_db, p.freq_hz))
            .unwrap_or((0.0, center_freq_hz as f64));

        SpectrumMetrics {
            noise_floor_db,
            peaks,
            peak_snr_db,
            peak_freq_hz,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Публичные функции
////////////////////////////////////////////////////////////////////////////////

/// Декодирует сырые байты IQ в вектор комплексных f32 выборок.
pub fn decode_iq(
    data: &[u8],
    format: IqFormat,
) -> Vec<Complex32> {
    match format {
        IqFormat::Int8 => data
            .chunks_exact(2)
            .map(|c| Complex32::new(c[0] as i8 as f32 / 128.0, c[1] as i8 as f32 / 128.0))
            .collect(),
        IqFormat::Int16 => data
            .chunks_exact(4)
            .map(|c| {
                let i = i16::from_be_bytes([c[0], c[1]]) as f32 / 32767.0;
                let q = i16::from_be_bytes([c[2], c[3]]) as f32 / 32767.0;

                Complex32::new(i, q)
            })
            .collect(),
        IqFormat::Float32 => data
            .chunks_exact(8)
            .map(|c| {
                let i = f32::from_be_bytes([c[0], c[1], c[2], c[3]]);
                let q = f32::from_be_bytes([c[4], c[5], c[6], c[7]]);

                Complex32::new(i, q)
            })
            .collect(),
    }
}

/// Отображает спектр мощности как ASCII-график в терминале.
pub fn render_ascii_spectrum(
    spectrum: &PowerSpectrum,
    metrics: &SpectrumMetrics,
    config: &SpectrumConfig,
    width: usize,
    height: usize,
) -> String {
    let power = &spectrum.power_db;
    let n_bins = power.len();

    if n_bins == 0 || width == 0 || height == 0 {
        return String::new();
    }

    let cols = width.min(n_bins);

    // Downsample: усреднение по группам бинов
    let group = n_bins.div_ceil(cols);
    let values: Vec<f32> = (0..cols)
        .filter_map(|c| {
            let start = c * group;
            if start >= n_bins {
                return None;
            }

            let end = ((c + 1) * group).min(n_bins);

            let avg = power[start..end].iter().sum::<f32>() / (end - start) as f32;
            Some(avg)
        })
        .collect();

    let min_db = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_db = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    let range = (max_db - min_db).max(1.0);
    let mut canvas = vec![vec![' '; cols]; height];

    for (col, &db) in values.iter().enumerate() {
        let normalized = ((db - min_db) / range).clamp(0.0, 1.0);
        let bar_h = (normalized * height as f32) as usize;

        for row in 0..bar_h.min(height) {
            canvas[height - 1 - row][col] = if row == bar_h.saturating_sub(1) {
                '▄'
            } else {
                '█'
            };
        }
    }

    let mut out = String::new();

    // Заголовок
    let center_mhz = config.center_freq_hz as f64 / 1e6;
    let bw_mhz = config.sample_rate_hz as f64 / 1e6;

    out.push_str(&format!(
        "┌─ Spectrum: {:.3} MHz ± {:.3} MHz  FFT:{} {}  noise:{:.1}dB  peak:{:.3}MHz SNR:{:.1}dB\n",
        center_mhz,
        bw_mhz / 2.0,
        config.fft_size,
        config.window,
        metrics.noise_floor_db,
        metrics.peak_freq_hz / 1e6,
        metrics.peak_snr_db,
    ));

    // db метки
    for (row_idx, row) in canvas.iter().enumerate() {
        let db_label = max_db - (row_idx as f32 / height as f32) * range;
        let line: String = row.iter().collect();

        if row_idx % (height / 4).max(1) == 0 {
            out.push_str(&format!("{:+6.1}│{}\n", db_label, line));
        } else {
            out.push_str(&format!("      │{}\n", line));
        }
    }

    // Ось частот
    let left_mhz = (config.center_freq_hz as f64 - config.sample_rate_hz as f64 / 2.0) / 1e6;
    let right_mhz = (config.center_freq_hz as f64 + config.sample_rate_hz as f64 / 2.0) / 1e6;

    out.push_str(&format!("      └{}\n", "─".repeat(cols)));
    out.push_str(&format!(
        "      {:<width$} {:.3} MHz\n",
        format!("{:.3} MHz", left_mhz),
        right_mhz,
        width = cols.saturating_sub(10)
    ));

    // Пики
    if !metrics.peaks.is_empty() {
        out.push_str("  Peaks: ");

        for p in metrics.peaks.iter().take(5) {
            out.push_str(&format!(
                "{:.3}MHz({:+.1}dB,SNR{:.1}dB)  ",
                p.freq_hz / 1e6,
                p.power_db,
                p.snr_db
            ));
        }

        out.push('\n');
    }

    out
}

/// Отображает waterfall как ASCII-тепловую карту.
pub fn render_ascii_waterfall(
    wf: &WaterfallBuffer,
    width: usize,
) -> String {
    let rows = wf.rows_ordered();

    if rows.is_empty() {
        return String::from("(waterfall empty)\n");
    }

    // Находим min/max по всей истории
    let (mut global_min, mut global_max) = (f32::INFINITY, f32::NEG_INFINITY);

    for row in &rows {
        for &v in *row {
            if v.is_finite() {
                global_min = global_min.min(v);
                global_max = global_max.max(v);
            }
        }
    }

    let range = (global_max - global_min).max(1.0);
    let palette = [' ', '░', '▒', '▓', '█'];

    let mut out = String::from("┌─ Waterfall (oldest→newest) ─────────────────\n");

    for row in &rows {
        out.push('│');

        let cols = width.min(row.len());
        let group = row.len().div_ceil(cols);

        for c in 0..cols {
            let start = c * group;

            if start >= row.len() {
                out.push(' ');
                continue;
            }

            let end = ((c + 1) * group).min(row.len());
            let avg = row[start..end].iter().sum::<f32>() / (end - start) as f32;
            let norm = ((avg - global_min) / range).clamp(0.0, 1.0);
            let idx = (norm * (palette.len() - 1) as f32) as usize;

            out.push(palette[idx]);
        }

        out.push('\n');
    }

    out.push('└');
    out.push_str(&"─".repeat(width + 1));
    out.push('\n');
    out
}

/// Экспортирует спектр в CSV строку.
pub fn export_spectrum_csv(
    spectrum: &PowerSpectrum,
    config: &SpectrumConfig,
) -> String {
    let freqs = spectrum.bin_frequencies(config.sample_rate_hz, config.center_freq_hz);
    let mut out = String::from("freq_hz,power_db\n");

    for (freq, &pwr) in freqs.iter().zip(spectrum.power_db.iter()) {
        out.push_str(&format!("{:.1},{:.4}\n", freq, pwr));
    }

    out
}

/// Экспортирует waterfall в CSV строку (строки = времени, столбцы = бины).
pub fn export_waterfall_csv(wf: &WaterfallBuffer) -> String {
    let rows = wf.rows_ordered();
    let mut out = String::new();

    for row in &rows {
        let line = row
            .iter()
            .map(|v| format!("{:.3}", v))
            .collect::<Vec<_>>()
            .join(",");

        out.push_str(&line);
        out.push('\n');
    }

    out
}

/// Записывает спектр мощности как PNG-изображение.
///
/// X-ось = частота, Y-ось = мощность (логарифмическая).
pub fn export_spectrum_png(
    spectrum: &PowerSpectrum,
    metrics: &SpectrumMetrics,
    _config: &SpectrumConfig,
    img_width: u32,
    img_height: u32,
) -> Result<Vec<u8>, String> {
    let power = &spectrum.power_db;
    let n_bins = power.len() as u32;

    let min_db = metrics.noise_floor_db - 5.0;
    let max_db = power
        .iter()
        .cloned()
        .fold(f32::NEG_INFINITY, f32::max)
        .max(min_db + 10.0);
    let range = (max_db - min_db).max(1.0);

    let mut img = ImageBuffer::<Rgb<u8>, _>::new(img_width, img_height);

    // Фон
    for p in img.pixels_mut() {
        *p = Rgb([20u8, 20, 30]);
    }

    // Спектральная кривая
    for x in 0..img_width {
        let bin_idx = (x as f32 * n_bins as f32 / img_width as f32) as usize;
        let bin_idx = bin_idx.min(power.len() - 1);
        let db = power[bin_idx];
        let norm = ((db - min_db) / range).clamp(0.0, 1.0);
        let y_top = (img_height as f32 * (1.0 - norm)) as u32;

        for y in y_top..img_height {
            let heat = (norm * 255.0) as u8;
            img.put_pixel(x, y, Rgb([heat, (heat / 2), 50]));
        }
    }

    // Отметить пики
    for peak in &metrics.peaks {
        let bin_f = (peak.bin_idx as f32 / power.len() as f32) * img_width as f32;
        let x_peak = bin_f as u32;
        let norm = ((peak.power_db - min_db) / range).clamp(0.0, 1.0);
        let y_peak = (img_height as f32 * (1.0 - norm)) as u32;
        for dy in 0..3u32 {
            if y_peak >= dy && x_peak < img_width {
                img.put_pixel(x_peak, y_peak.saturating_sub(dy), Rgb([255, 255, 0]));
            }
        }
    }

    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(
            img.as_raw(),
            img_width,
            img_height,
            image::ColorType::Rgb8.into(),
        )
        .map_err(|e| format!("PNG encode error: {e}"))?;

    Ok(buf)
}

/// Записывает waterfall как PNG-тепловую карту.
pub fn export_waterfall_png(
    wf: &WaterfallBuffer,
    img_width: u32,
    img_height: u32,
) -> Result<Vec<u8>, String> {
    let rows = wf.rows_ordered();
    if rows.is_empty() {
        return Err("Waterfall is empty".to_string());
    }

    let (mut global_min, mut global_max) = (f32::INFINITY, f32::NEG_INFINITY);
    for row in &rows {
        for &v in *row {
            if v.is_finite() {
                global_min = global_min.min(v);
                global_max = global_max.max(v);
            }
        }
    }
    let range = (global_max - global_min).max(1.0);

    let n_rows = rows.len() as u32;
    let n_cols = wf.cols() as u32;

    let mut img = ImageBuffer::<Rgb<u8>, _>::new(img_width, img_height);

    for py in 0..img_height {
        let row_idx = (py as f32 * n_rows as f32 / img_height as f32) as usize;
        let row_idx = row_idx.min(rows.len() - 1);
        let row = rows[row_idx];

        for px in 0..img_width {
            let col_idx = (px as f32 * n_cols as f32 / img_width as f32) as usize;
            let col_idx = col_idx.min(row.len() - 1);
            let v = row[col_idx];
            let norm = if v.is_finite() {
                ((v - global_min) / range).clamp(0.0, 1.0)
            } else {
                0.0
            };

            // Тепловая карта: синий → зелёный → красный
            let (r, g, b) = heat_color(norm);
            img.put_pixel(px, py, Rgb([r, g, b]));
        }
    }

    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(
            img.as_raw(),
            img_width,
            img_height,
            image::ColorType::Rgb8.into(),
        )
        .map_err(|e| format!("PNG encode error: {e}"))?;

    Ok(buf)
}

////////////////////////////////////////////////////////////////////////////////
// Внутренние функции
////////////////////////////////////////////////////////////////////////////////

/// Медиана вектора f32 (не изменяет исходный).
fn median(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }

    let mut sorted = v.to_vec();

    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

    let mid = sorted.len() / 2;

    if sorted.len().is_multiple_of(2) {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Тепловая карта 0.0 = синий, 0.5 = зелёный, 1.0 = красный.
fn heat_color(t: f32) -> (u8, u8, u8) {
    let r = (255.0 * (t * 2.0 - 1.0).clamp(0.0, 1.0)) as u8;
    let g = (255.0 * (1.0 - (t * 2.0 - 1.0).abs()).clamp(0.0, 1.0)) as u8;
    let b = (255.0 * (1.0 - t * 2.0).clamp(0.0, 1.0)) as u8;

    (r, g, b)
}

////////////////////////////////////////////////////////////////////////////////
// Общие реализации трейтов для SpectrumConfig, WindowFunction
////////////////////////////////////////////////////////////////////////////////

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            fft_size: 1024,
            window: WindowFunction::Hann,
            avg_count: 8,
            waterfall_rows: 50,
            sample_rate_hz: 2_000_000,
            center_freq_hz: 1_602_000_000,
        }
    }
}

impl Default for PeakDetector {
    fn default() -> Self {
        Self {
            min_snr_db: 10.0,
            min_bin_distance: 5,
        }
    }
}

impl std::str::FromStr for WindowFunction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rect" | "rectangular" | "none" => Ok(WindowFunction::Rectangular),
            "hann" | "hanning" => Ok(WindowFunction::Hann),
            "blackman" => Ok(WindowFunction::Blackman),
            _ => Err(format!("Unknown window '{s}'. Use: rect, hann, blackman")),
        }
    }
}

impl std::fmt::Display for WindowFunction {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            WindowFunction::Rectangular => write!(f, "Rectangular"),
            WindowFunction::Hann => write!(f, "Hann"),
            WindowFunction::Blackman => write!(f, "Blackman"),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Тесты
////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use glos_types::IqFormat;
    use rand::{rngs::SmallRng, Rng, SeedableRng};
    use rustfft::num_complex::Complex32;

    use crate::{
        decode_iq, export_spectrum_csv, export_spectrum_png, export_waterfall_csv,
        export_waterfall_png, render_ascii_spectrum, render_ascii_waterfall, spectrum::median,
        PeakDetector, PowerSpectrum, SpectrumConfig, SpectrumMetrics, SpectrumProcessor,
        WaterfallBuffer, WindowFunction,
    };

    fn make_tone_iq(
        freq_hz: f32,
        sample_rate: u32,
        n: usize,
    ) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let phase = 2.0 * PI * freq_hz * t;
                Complex32::new(phase.cos(), phase.sin())
            })
            .collect()
    }

    #[test]
    fn test_window_coefficients_hann_endpoint() {
        let w = WindowFunction::Hann.coefficients(1024);

        assert!(w[0].abs() < 1e-3, "Hann[0] must be close to 0");
        assert!(w[1023].abs() < 1e-3, "Hann[N-1] must be close to 0");
        assert!(
            (w[512] - 1.0).abs() < 0.01,
            "Hannp[N/2] should be close to 1"
        );
    }

    #[test]
    fn test_window_coefficients_blackman() {
        let w = WindowFunction::Blackman.coefficients(512);

        assert_eq!(w.len(), 512);
        assert!(w[0].abs() < 0.01);
    }

    #[test]
    fn test_window_fromstr() {
        assert_eq!(
            "hann".parse::<WindowFunction>().unwrap(),
            WindowFunction::Hann
        );
        assert_eq!(
            "blackman".parse::<WindowFunction>().unwrap(),
            WindowFunction::Blackman
        );
        assert_eq!(
            "rect".parse::<WindowFunction>().unwrap(),
            WindowFunction::Rectangular
        );
        assert!("unknown".parse::<WindowFunction>().is_err());
    }

    #[test]
    fn test_decode_iq_int8() {
        let data = vec![127i8 as u8, 0u8, 0u8, 127i8 as u8];
        let decoded = decode_iq(&data, IqFormat::Int8);

        assert_eq!(decoded.len(), 2);
        assert!(decoded[0].re > 0.99);
        assert!(decoded[1].im > 0.99);
    }

    #[test]
    fn test_decode_iq_int16() {
        // Оригинальные float-сэмплы
        let samples = vec![
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, -1.0),
            Complex32::new(0.5, 0.5),
            Complex32::new(-0.75, 0.25),
        ];

        // Кодируем вручную в int16 BE
        let mut data = Vec::new();

        for s in &samples {
            let i = (s.re * 32767.0) as i16;
            let q = (s.im * 32767.0) as i16;

            data.extend_from_slice(&i.to_be_bytes());
            data.extend_from_slice(&q.to_be_bytes());
        }

        // Декодируем
        let decoded = decode_iq(&data, IqFormat::Int16);

        assert_eq!(decoded.len(), samples.len());

        for (orig, dec) in samples.iter().zip(decoded.iter()) {
            assert!((orig.re - dec.re).abs() < 1e-4);
            assert!((orig.im - dec.im).abs() < 1e-4);
        }
    }

    #[test]
    fn test_decode_iq_float32() {
        // Оригинальные float-сэмплы
        let samples = vec![
            Complex32::new(1.0, -1.0),
            Complex32::new(0.5, 0.25),
            Complex32::new(-0.75, 0.125),
        ];

        let mut data = Vec::new();

        for s in &samples {
            data.extend_from_slice(&s.re.to_be_bytes());
            data.extend_from_slice(&s.im.to_be_bytes());
        }

        let decoded = decode_iq(&data, IqFormat::Float32);

        assert_eq!(decoded.len(), samples.len());

        for (orig, dec) in samples.iter().zip(decoded.iter()) {
            assert!((orig.re - dec.re).abs() < 1e-6);
            assert!((orig.im - dec.im).abs() < 1e-6);
        }
    }

    #[test]
    fn test_decode_iq_int16_roundtrip() {
        // Генерируем int16 BE IQ
        let samples = vec![
            Complex32::new(1.0, 0.0),
            Complex32::new(0.0, -1.0),
            Complex32::new(0.5, 0.5),
        ];

        let mut data = Vec::new();

        for s in &samples {
            let i = (s.re * 32767.0) as i16;
            let q = (s.im * 32767.0) as i16;

            data.extend_from_slice(&i.to_be_bytes());
            data.extend_from_slice(&q.to_be_bytes());
        }

        let decoded = decode_iq(&data, IqFormat::Int16);

        assert_eq!(decoded.len(), 3);
        assert!((decoded[0].re - 1.0).abs() < 1e-4, "I[0] ≈ 1.0");
        assert!(decoded[0].im.abs() < 1e-4, "Q[0] ≈ 0.0");
    }

    #[test]
    fn test_decode_iq_int8_extremes() {
        let data = vec![127i8 as u8, 127i8 as u8, (-128i8) as u8, (-128i8) as u8];

        let decoded = decode_iq(&data, IqFormat::Int8);

        // max
        assert!(decoded[0].re > 0.99);
        assert!(decoded[0].im > 0.99);

        // min
        assert!(decoded[1].re <= -1.0);
        assert!(decoded[1].im <= -1.0);
    }

    #[test]
    fn test_decode_iq_int16_extremes() {
        let mut data = Vec::new();

        let max = 32767i16;
        let min = -32768i16;

        data.extend_from_slice(&max.to_be_bytes());
        data.extend_from_slice(&max.to_be_bytes());
        data.extend_from_slice(&min.to_be_bytes());
        data.extend_from_slice(&min.to_be_bytes());

        let decoded = decode_iq(&data, IqFormat::Int16);

        assert_eq!(decoded.len(), 2);
        assert!((decoded[0].re - 1.0).abs() < 1e-6);
        assert!(decoded[1].re < -1.0);
    }

    #[test]
    fn test_decode_iq_ignores_trailing_bytes() {
        let data = vec![127i8 as u8, 0u8, 1u8];

        let decoded = decode_iq(&data, IqFormat::Int8);

        // Должна жекодироваться только одна пара
        assert_eq!(decoded.len(), 1);
    }

    #[test]
    fn test_window_power_norm_positive() {
        let n = 1024;
        let rect = WindowFunction::Rectangular.power_norm(n);
        let hann = WindowFunction::Hann.power_norm(n);
        let black = WindowFunction::Blackman.power_norm(n);

        assert!(rect > hann);
        assert!(hann > black);
    }

    #[test]
    fn test_bin_frequencies_center() {
        let spectrum = PowerSpectrum {
            power_db: vec![0.0; 4],
            timestamp_ns: 0,
        };
        let fregs = spectrum.bin_frequencies(4, 1000);

        // Для n = 4: bin: -2, -1, 0, 1
        // width = 4 / 4 = 1 Гц

        assert_eq!(fregs.len(), 4);
        assert_eq!(fregs[2], 1000.0); // центральный бин
    }

    #[test]
    fn test_window_symmetry() {
        let n = 256;
        let w = WindowFunction::Hann.coefficients(n);

        for i in 0..n {
            assert!((w[i] - w[n - 1 - i]).abs() < 1e-6);
        }
    }

    #[test]
    fn test_spectrum_processor_new_initialization() {
        let cfg = SpectrumConfig {
            fft_size: 512,
            window: WindowFunction::Hann,
            avg_count: 4,
            waterfall_rows: 10,
            sample_rate_hz: 1_000_000,
            center_freq_hz: 100_000_000,
        };

        let proc = SpectrumProcessor::new(cfg.clone());

        assert_eq!(proc.window_coeff.len(), 512);
        assert_eq!(proc.avg_acc.len(), 512);
        assert_eq!(proc.avg_filled, 0);
        assert!(proc.power_norm > 0.0);
    }

    #[test]
    fn test_rectangular_power_norm_exact() {
        let n = 256;
        let norm = WindowFunction::Rectangular.power_norm(n);

        assert!((norm - n as f32).abs() < 1e-6);
    }

    #[test]
    fn test_process_block_too_small() {
        let cfg = SpectrumConfig {
            fft_size: 1024,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);
        let samples = vec![Complex32::new(0.0, 0.0); 100];
        let res = proc.process_block(&samples, 0);

        assert!(res.is_none());
    }

    #[test]
    fn test_process_block_avg_count() {
        let cfg = SpectrumConfig {
            fft_size: 256,
            avg_count: 2,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);

        let samples = vec![Complex32::new(0.0, 0.0); 256];

        let r1 = proc.process_block(&samples, 0);
        let r2 = proc.process_block(&samples, 1);

        assert!(r1.is_none());
        assert!(r2.is_some());
    }

    #[test]
    fn test_process_block_output_size() {
        let cfg = SpectrumConfig {
            fft_size: 512,
            avg_count: 1,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);
        let samples = vec![Complex32::new(0.0, 0.0); 512];
        let res = proc.process_block(&samples, 0).unwrap();

        assert_eq!(res.power_db.len(), 512);
    }

    #[test]
    fn test_fftshift_dc_center() {
        let cfg = SpectrumConfig {
            fft_size: 128,
            avg_count: 1,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);
        let samples = vec![Complex32::new(1.0, 0.0); 128];
        let res = proc.process_block(&samples, 0).unwrap();
        let center = 128 / 2;
        let max_bin = res
            .power_db
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;

        assert_eq!(max_bin, center);
    }

    #[test]
    fn test_process_multiple_windows() {
        let cfg = SpectrumConfig {
            fft_size: 128,
            avg_count: 4,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);
        let samples = vec![Complex32::new(0.0, 0.0); 512];
        let res = proc.process_block(&samples, 0);

        assert!(res.is_some());
    }

    #[test]
    fn test_timestamp_propagation() {
        let cfg = SpectrumConfig {
            fft_size: 128,
            avg_count: 1,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);
        let samples = vec![Complex32::new(0.0, 0.0); 128];
        let ts = 123456;
        let res = proc.process_block(&samples, ts).unwrap();

        assert_eq!(res.timestamp_ns, ts);
    }

    #[test]
    fn test_power_no_nan() {
        let cfg = SpectrumConfig {
            fft_size: 256,
            avg_count: 1,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);

        let samples = vec![Complex32::new(0.0, 0.0); 256];

        let res = proc.process_block(&samples, 0).unwrap();

        for v in res.power_db {
            assert!(v.is_finite());
        }
    }

    #[test]
    fn test_reset_clears_accumulator() {
        let cfg = SpectrumConfig {
            fft_size: 128,
            avg_count: 4,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);
        let samples = vec![Complex32::new(0.0, 0.0); 128];

        // заполняем аккумулятор
        proc.process_block(&samples, 0);
        proc.process_block(&samples, 0);

        assert!(proc.avg_filled > 0);

        proc.reset();

        assert_eq!(proc.avg_filled, 0);
        assert!(proc.avg_acc.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn test_config_accessor() {
        let cfg = SpectrumConfig {
            fft_size: 256,
            avg_count: 3,
            ..Default::default()
        };

        let proc = SpectrumProcessor::new(cfg.clone());

        let c = proc.config();

        assert_eq!(c.fft_size, cfg.fft_size);
        assert_eq!(c.avg_count, cfg.avg_count);
    }

    #[test]
    fn test_reset_resets_averaging() {
        let cfg = SpectrumConfig {
            fft_size: 128,
            avg_count: 2,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(cfg);

        let samples = vec![Complex32::new(0.0, 0.0); 128];

        proc.process_block(&samples, 0);
        proc.reset();

        // после reset первый вызов снова должен вернуть None
        let res = proc.process_block(&samples, 0);

        assert!(res.is_none());
    }

    #[test]
    fn test_waterfall_init() {
        let wf = WaterfallBuffer::new(5, 8);

        assert_eq!(wf.filled_rows(), 0);
        assert_eq!(wf.cols(), 8);
    }

    #[test]
    fn test_waterfall_push_single() {
        let mut wf = WaterfallBuffer::new(3, 4);

        wf.push(&[1.0, 2.0, 3.0, 4.0]);

        assert_eq!(wf.filled_rows(), 1);

        let rows = wf.rows_ordered();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0], &[1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_waterfall_multiple_rows() {
        let mut wf = crate::WaterfallBuffer::new(5, 3);

        wf.push(&[1.0, 1.0, 1.0]);
        wf.push(&[2.0, 2.0, 2.0]);
        wf.push(&[3.0, 3.0, 3.0]);

        let rows = wf.rows_ordered();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], &[1.0, 1.0, 1.0]);
        assert_eq!(rows[1], &[2.0, 2.0, 2.0]);
        assert_eq!(rows[2], &[3.0, 3.0, 3.0]);
    }

    #[test]
    fn test_waterfall_ring_overwrite() {
        let mut wf = crate::WaterfallBuffer::new(3, 2);

        wf.push(&[1.0, 1.0]);
        wf.push(&[2.0, 2.0]);
        wf.push(&[3.0, 3.0]);
        wf.push(&[4.0, 4.0]); // перезапишет первую строку

        let rows = wf.rows_ordered();

        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0], &[2.0, 2.0]);
        assert_eq!(rows[1], &[3.0, 3.0]);
        assert_eq!(rows[2], &[4.0, 4.0]);
    }

    #[test]
    fn test_waterfall_truncate_input() {
        let mut wf = crate::WaterfallBuffer::new(2, 3);

        wf.push(&[1.0, 2.0, 3.0, 4.0, 5.0]);

        let rows = wf.rows_ordered();

        assert_eq!(rows[0], &[1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_waterfall_partial_fill() {
        let mut wf = crate::WaterfallBuffer::new(5, 2);

        wf.push(&[1.0, 1.0]);
        wf.push(&[2.0, 2.0]);

        assert_eq!(wf.filled_rows(), 2);

        let rows = wf.rows_ordered();

        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn test_waterfall_order_after_wrap() {
        let mut wf = crate::WaterfallBuffer::new(3, 1);

        wf.push(&[1.0]);
        wf.push(&[2.0]);
        wf.push(&[3.0]);
        wf.push(&[4.0]);
        wf.push(&[5.0]);

        let rows = wf.rows_ordered();

        assert_eq!(rows.len(), 3);

        assert_eq!(rows[0], &[3.0]);
        assert_eq!(rows[1], &[4.0]);
        assert_eq!(rows[2], &[5.0]);
    }

    #[test]
    fn test_median_odd_len() {
        let v = vec![1.0, 5.0, 3.0];
        let m = median(&v);

        assert_eq!(m, 3.0);
    }

    #[test]
    fn test_median_even_len() {
        let v = vec![1.0, 2.0, 3.0, 4.0];
        let m = median(&v);

        assert_eq!(m, 2.5);
    }

    #[test]
    fn test_median_single() {
        let v = vec![42.0];
        let m = median(&v);

        assert_eq!(m, 42.0);
    }

    #[test]
    fn test_median_empty() {
        let v: Vec<f32> = vec![];
        let m = median(&v);

        assert_eq!(m, 0.0);
    }

    #[test]
    fn test_median_calculation() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 3.0, 2.0]), 2.5);
        assert_eq!(median(&[5.0]), 5.0);
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn test_peak_detector_single_peak() {
        let spectrum = PowerSpectrum {
            power_db: vec![-50.0, -49.0, -48.0, -10.0, -47.0, -48.0],
            timestamp_ns: 0,
        };
        let det = PeakDetector::new(5.0, 1);
        let metrics = det.analyze(&spectrum, 1_000_000, 100_000_000);

        assert_eq!(metrics.peaks.len(), 1);
        assert!(metrics.peak_snr_db > 30.0);
    }

    #[test]
    fn test_peak_detector_threshold() {
        let spectrum = PowerSpectrum {
            power_db: vec![-50.0, -49.0, -48.0, -45.0, -49.0],
            timestamp_ns: 0,
        };
        let det = PeakDetector::new(10.0, 1);
        let metrics = det.analyze(&spectrum, 1_000_000, 100_000_000);

        assert!(metrics.peaks.is_empty());
    }

    #[test]
    fn test_peak_detector_min_bin_distance() {
        // Два локальных максимума рядом (индексы 1 и 3),
        // оба должны пройти SNR-порог, но расстояние = 2 < min_bin_distance(3),
        // значит в результате останется ровно один пик.
        let spectrum = PowerSpectrum {
            power_db: vec![-50.0, -5.0, -6.0, -4.0, -50.0],
            timestamp_ns: 0,
        };
        // Небольшой порог SNR, чтобы пики -5 и -4 прошли:
        let det = PeakDetector::new(0.5, 3);
        let metrics = det.analyze(&spectrum, 1_000_000, 100_000_000);

        assert_eq!(
            metrics.peaks.len(),
            1,
            "We expected one peak after the suppression of nearby peaks."
        );
        // Опционально — проверить, что оставленный пик это сильнейший (индекс 3, power
        // -4.0)
        assert_eq!(metrics.peaks[0].bin_idx, 3);
        assert!((metrics.peaks[0].power_db - (-4.0)).abs() < 1e-6);
    }

    #[test]
    fn test_peak_detector_strongest_peak() {
        let spectrum = PowerSpectrum {
            power_db: vec![-50.0, -20.0, -10.0, -30.0, -5.0, -50.0],
            timestamp_ns: 0,
        };
        let det = PeakDetector::new(5.0, 1);
        let metrics = det.analyze(&spectrum, 1_000_000, 100_000_000);

        assert!(metrics.peak_snr_db > 15.0);
    }

    #[test]
    fn test_peak_detector_frequency_mapping() {
        let spectrum = PowerSpectrum {
            power_db: vec![-50.0, -50.0, -5.0, -50.0],
            timestamp_ns: 0,
        };
        let det = PeakDetector::new(5.0, 1);
        let metrics = det.analyze(&spectrum, 4, 1000);

        // бин 2 — центр
        assert!((metrics.peak_freq_hz - 1000.0).abs() < 1.0);
    }

    #[test]
    fn test_peak_detector_no_peaks() {
        let spectrum = PowerSpectrum {
            power_db: vec![-50.0; 16],
            timestamp_ns: 0,
        };
        let det = PeakDetector::new(5.0, 1);
        let metrics = det.analyze(&spectrum, 1_000_000, 100_000_000);

        assert!(metrics.peaks.is_empty());
        assert_eq!(metrics.peak_snr_db, 0.0);
    }

    #[test]
    fn test_spectrum_processor_tone_detected() {
        // Тон на 100 кГц при 2 Msps, FFT 1024
        let sample_rate = 2_000_000u32;
        let tone_hz = 100_000.0f32;
        let config = SpectrumConfig {
            fft_size: 1024,
            window: WindowFunction::Hann,
            avg_count: 1,
            waterfall_rows: 10,
            sample_rate_hz: sample_rate,
            center_freq_hz: 1_602_000_000,
        };

        let mut proc = SpectrumProcessor::new(config.clone());
        let samples = make_tone_iq(tone_hz, sample_rate, 4096);
        let spectrum = proc
            .process_block(&samples, 0)
            .expect("must return the spectrum");

        assert_eq!(spectrum.power_db.len(), 1024);

        // Находим бин с максимальной мощностью
        let max_bin = spectrum
            .power_db
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // Ожидаем бин: tone_hz / (sample_rate / fft_size) + fft_size / 2
        let bin_width = sample_rate as f32 / 1024.0;
        let expected_bin = (1024 / 2 + (tone_hz / bin_width) as usize).min(1023);
        let bin_tolerance = 3;

        assert!(
            max_bin.abs_diff(expected_bin) <= bin_tolerance,
            "Tone must be in bin {expected_bin} ±{bin_tolerance}, bin {max_bin} found"
        );
    }

    #[test]
    fn test_spectrum_processor_averaging() {
        let config = SpectrumConfig {
            fft_size: 512,
            avg_count: 4,
            window: WindowFunction::Hann,
            waterfall_rows: 10,
            sample_rate_hz: 2_000_000,
            center_freq_hz: 1_602_000_000,
        };
        let mut proc = SpectrumProcessor::new(config);

        // Первый вызов с avg_count=4: нужно как минимум 4 окна → 512*2*4=4096 samples
        let samples = make_tone_iq(50_000.0, 2_000_000, 8192);
        let result = proc.process_block(&samples, 0);

        assert!(
            result.is_some(),
            "with enough data it should return the spectrum"
        );
    }

    #[test]
    fn test_waterfall_buffer_ring() {
        let mut wf = WaterfallBuffer::new(3, 8);

        assert_eq!(wf.filled_rows(), 0);

        wf.push(&[1.0f32; 8]);
        wf.push(&[2.0f32; 8]);
        wf.push(&[3.0f32; 8]);

        assert_eq!(wf.filled_rows(), 3);

        // Четвёртая строка вытесняет первую
        wf.push(&[4.0f32; 8]);

        assert_eq!(wf.filled_rows(), 3);

        let rows = wf.rows_ordered();

        // oldest first: [2, 3, 4]
        assert!((rows[0][0] - 2.0).abs() < 1e-6);
        assert!((rows[2][0] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_peak_detector_finds_tone() {
        let config = SpectrumConfig {
            fft_size: 1024,
            avg_count: 1,
            window: WindowFunction::Hann,
            waterfall_rows: 10,
            sample_rate_hz: 2_000_000,
            center_freq_hz: 1_602_000_000,
        };
        let mut proc = SpectrumProcessor::new(config.clone());
        let samples = make_tone_iq(200_000.0, 2_000_000, 4096);
        let spectrum = proc.process_block(&samples, 0).unwrap();

        let detector = PeakDetector::new(10.0, 5);
        let metrics = detector.analyze(&spectrum, config.sample_rate_hz, config.center_freq_hz);

        assert!(!metrics.peaks.is_empty(), "must find at least one peak");
        assert!(metrics.peak_snr_db > 10.0, "Tone SNR should be > 10 dB");

        // Пик должен быть близко к 200 кГц от несущей (в пределах 2 бина)
        let expected_freq = 1_602_000_000.0 + 200_000.0;
        let bin_width = 2_000_000.0 / 1024.0;
        assert!(
            (metrics.peak_freq_hz - expected_freq).abs() < bin_width * 3.0,
            "Peak should be close to {:.0} Hz, found {:.0} Hz",
            expected_freq,
            metrics.peak_freq_hz
        );
    }

    #[test]
    fn test_peak_detector_noise_floor() {
        // Белый шум: пики не должны превышать порог
        let mut rng = SmallRng::seed_from_u64(42);
        let samples: Vec<Complex32> = (0..4096)
            .map(|_| Complex32::new(rng.gen_range(-0.01f32..0.01), rng.gen_range(-0.01f32..0.01)))
            .collect();

        let config = SpectrumConfig {
            fft_size: 1024,
            avg_count: 1,
            ..Default::default()
        };
        let mut proc = SpectrumProcessor::new(config.clone());
        let spectrum = proc.process_block(&samples, 0).unwrap();

        let detector = PeakDetector::new(20.0, 5); // высокий порог
        let metrics = detector.analyze(&spectrum, config.sample_rate_hz, config.center_freq_hz);

        assert!(
            metrics.peaks.is_empty() || metrics.peak_snr_db < 30.0,
            "White noise should not produce peaks with SNR > 30 dB."
        );
    }

    #[test]
    fn test_export_spectrum_csv_format() {
        let spectrum = PowerSpectrum {
            power_db: vec![-60.0, -55.0, -50.0, -55.0],
            timestamp_ns: 0,
        };
        let config = SpectrumConfig {
            fft_size: 4,
            sample_rate_hz: 1_000_000,
            center_freq_hz: 1_000_000_000,
            ..Default::default()
        };
        let csv = export_spectrum_csv(&spectrum, &config);
        assert!(csv.starts_with("freq_hz,power_db\n"));
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 5, "header + 4 lines");
    }

    #[test]
    fn test_export_waterfall_csv() {
        let mut wf = WaterfallBuffer::new(3, 4);
        wf.push(&[1.0, 2.0, 3.0, 4.0]);
        wf.push(&[5.0, 6.0, 7.0, 8.0]);
        let csv = export_waterfall_csv(&wf);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("1.000"));
    }

    #[test]
    fn test_ascii_spectrum_renders() {
        let config = SpectrumConfig {
            fft_size: 64,
            avg_count: 1,
            ..Default::default()
        };
        let mut proc = SpectrumProcessor::new(config.clone());
        let samples = make_tone_iq(100_000.0, 2_000_000, 512);
        let spectrum = proc.process_block(&samples, 0).unwrap();
        let detector = PeakDetector::default();
        let metrics = detector.analyze(&spectrum, config.sample_rate_hz, config.center_freq_hz);
        let rendered = render_ascii_spectrum(&spectrum, &metrics, &config, 60, 10);
        assert!(rendered.contains("Spectrum"), "must contain header");
        assert!(
            rendered.contains('█') || rendered.contains('▄'),
            "must contain columns"
        );
    }

    #[test]
    fn test_ascii_waterfall_renders() {
        let mut wf = WaterfallBuffer::new(5, 64);
        for i in 0..5u32 {
            wf.push(&vec![i as f32 * 10.0 - 80.0; 64]);
        }
        let rendered = render_ascii_waterfall(&wf, 40);
        assert!(rendered.contains("Waterfall"));
        assert!(rendered.lines().count() > 3);
    }

    #[test]
    fn test_bin_frequencies_correct() {
        let spectrum = PowerSpectrum {
            power_db: vec![0.0; 4],
            timestamp_ns: 0,
        };
        let freqs = spectrum.bin_frequencies(4, 10);
        // При 4 бинах и sample_rate=4: бины [-2, -1, 0, 1] * 1Hz + center 10
        // = [8, 9, 10, 11]
        assert_eq!(freqs, vec![8.0, 9.0, 10.0, 11.0]);
    }

    #[test]
    fn test_decode_iq_empty_input() {
        let data: Vec<u8> = vec![];

        let r1 = decode_iq(&data, IqFormat::Int8);
        let r2 = decode_iq(&data, IqFormat::Int16);
        let r3 = decode_iq(&data, IqFormat::Float32);

        assert!(r1.is_empty());
        assert!(r2.is_empty());
        assert!(r3.is_empty());
    }

    #[test]
    fn test_spectrum_processor_noise_stability() {
        let mut rng = SmallRng::seed_from_u64(123);
        let samples: Vec<Complex32> = (0..4096)
            .map(|_| Complex32::new(rng.gen_range(-0.5..0.5), rng.gen_range(-0.5..0.5)))
            .collect();

        let config = SpectrumConfig {
            fft_size: 1024,
            avg_count: 1,
            ..Default::default()
        };

        let mut proc = SpectrumProcessor::new(config);
        let spectrum = proc.process_block(&samples, 0).unwrap();

        for p in spectrum.power_db {
            assert!(p.is_finite());
        }
    }

    #[test]
    fn test_export_waterfall_png() {
        let mut wf = WaterfallBuffer::new(4, 8);

        wf.push(&[-60.0; 8]);
        wf.push(&[-50.0; 8]);
        wf.push(&[-40.0; 8]);

        let png = export_waterfall_png(&wf, 200, 100).unwrap();

        assert!(png.len() > 100);
    }

    #[test]
    fn test_export_spectrum_png() {
        let spectrum = PowerSpectrum {
            power_db: vec![-60.0, -40.0, -10.0, -40.0, -60.0],
            timestamp_ns: 0,
        };

        let metrics = SpectrumMetrics {
            noise_floor_db: -60.0,
            peaks: vec![],
            peak_snr_db: 0.0,
            peak_freq_hz: 0.0,
        };

        let config = SpectrumConfig::default();

        let png = export_spectrum_png(&spectrum, &metrics, &config, 300, 200).unwrap();

        assert!(png.len() > 100);
    }

    #[test]
    fn test_ascii_spectrum_small_size() {
        let spectrum = PowerSpectrum {
            power_db: vec![-50.0; 16],
            timestamp_ns: 0,
        };

        let metrics = SpectrumMetrics {
            noise_floor_db: -50.0,
            peaks: vec![],
            peak_snr_db: 0.0,
            peak_freq_hz: 0.0,
        };

        let config = SpectrumConfig::default();

        let rendered = render_ascii_spectrum(&spectrum, &metrics, &config, 1, 1);

        assert!(!rendered.is_empty());
    }
}

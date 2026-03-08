use std::{f32::consts::PI, sync::Arc};

use glos_types::IqFormat;
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
    use glos_types::IqFormat;
    use rustfft::num_complex::Complex32;

    use crate::{
        decode_iq, PowerSpectrum, SpectrumConfig, SpectrumProcessor, WaterfallBuffer,
        WindowFunction,
    };

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
}

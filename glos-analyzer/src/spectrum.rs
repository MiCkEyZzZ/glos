use std::f32::consts::PI;

use glos_types::IqFormat;
use rustfft::num_complex::Complex32;

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

/// Декодирует сырые байты IQ в вектор комплексных f32 выборок.
pub fn decode_id(
    data: &[u8],
    format: IqFormat,
) -> Vec<Complex32> {
    match format {
        IqFormat::Int8 => data
            .chunks_exact(2)
            .map(|c| Complex32::new(c[0] as i8 as f32 / 127.0, c[1] as i8 as f32 / 127.0))
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

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            fft_size: 1024,
            window: WindowFunction::Hann,
            avg_count: 8,
            waterfall_rows: 50,
            sample_rate_hz: 2_000_000,
            center_freq_hz: 16_020_000_000,
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

#[cfg(test)]
mod tests {
    use glos_types::IqFormat;
    use rustfft::num_complex::Complex32;

    use crate::{decode_id, WindowFunction};

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
        let decoded = decode_id(&data, IqFormat::Int8);

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
        let decoded = decode_id(&data, IqFormat::Int16);

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

        let decoded = decode_id(&data, IqFormat::Float32);

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

        let decoded = decode_id(&data, IqFormat::Int16);

        assert_eq!(decoded.len(), 3);
        assert!((decoded[0].re - 1.0).abs() < 1e-4, "I[0] ≈ 1.0");
        assert!(decoded[0].im.abs() < 1e-4, "Q[0] ≈ 0.0");
    }

    #[test]
    fn test_decode_iq_int8_extremes() {
        let data = vec![
            127i8 as u8,
            127i8 as u8, // max
            (-128i8) as u8,
            (-128i8) as u8, // min
        ];

        let decoded = decode_id(&data, IqFormat::Int8);

        // max
        assert!((decoded[0].re - 1.0).abs() < 1e-6);
        assert!((decoded[0].im - 1.0).abs() < 1e-6);

        // min (будет приблизительно = 1.0078 из-за / 127.0)
        assert!(decoded[1].re < -1.0);
        assert!(decoded[1].im < -1.0);
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

        let decoded = decode_id(&data, IqFormat::Int16);

        assert_eq!(decoded.len(), 2);
        assert!((decoded[0].re - 1.0).abs() < 1e-6);
        assert!(decoded[1].re < -1.0);
    }
}

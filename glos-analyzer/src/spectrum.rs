use std::f32::consts::PI;

/// Оконная ф-я для FFT
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
    use crate::WindowFunction;

    #[test]
    fn test_window_coefficients_hann_endpoint() {
        let w = WindowFunction::Hann.coefficients(1024);

        assert!(w[0].abs() < 1e-3, "Hann[0] must be close to 0");
    }
}

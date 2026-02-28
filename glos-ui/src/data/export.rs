use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use chrono::{DateTime, Utc};
use serde_json::json;

use super::{AppState, Satellite};

pub struct DataExporter;

impl DataExporter {
    /// Экспорт спутника в CSV.
    pub fn export_satellites_csv(
        satellites: &[Satellite],
        timestamp: DateTime<Utc>,
        path: &Path,
    ) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Заголовок
        writeln!(
            writer,
            "timestamp,id,constellation,cn0_dbhz,elevation_deg,azimuth_deg,doppler_hz,used_in_fix"
        )?;

        // Данные
        for sat in satellites {
            writeln!(
                writer,
                "{},{},{},{:.2},{:.2},{:.2},{:.2},{}",
                timestamp.to_rfc3339(),
                sat.id,
                sat.constellation,
                sat.cn0,
                sat.elevation,
                sat.azimuth,
                sat.doppler,
                sat.used_in_fix as u8,
            )?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Экспорт FFT спектра в CSV.
    pub fn export_fft_csv(
        fft_data: &[f32],
        frequency_mhz: f32,
        sample_rate_mhz: f32,
        timestamp: DateTime<Utc>,
        path: &Path,
    ) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "# Timestamp: {}", timestamp.to_rfc3339())?;
        writeln!(writer, "# Center Frequency: {frequency_mhz:.2} Мгц")?;
        writeln!(writer, "# Sample Rate: {sample_rate_mhz:.2} Мгц")?;
        writeln!(writer, "frequency_mhz,power_db")?;

        for (i, power) in fft_data.iter().enumerate() {
            let freq = (i as f32 / fft_data.len() as f32 - 0.5) * sample_rate_mhz + frequency_mhz;
            writeln!(writer, "{freq:.6},{power:.2}")?;
        }

        writer.flush()?;
        Ok(())
    }

    /// Экспорт скриншота (через egui)
    pub fn export_screenshot(
        _ctx: &egui::Context,
        _path: &Path,
    ) -> Result<(), String> {
        // В egui нужно использовать специальный механизм
        // Пока заглушка - реализуется через ctx.request_screenshot()
        Err("Screenshot export not yet implemented".to_string())
    }

    /// Создание JSON-отчёта.
    pub fn export_session_report(
        state: &AppState,
        path: &Path,
    ) -> std::io::Result<()> {
        let report = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "status": format!("{:?}", state.status),
            "satellites": {
                "total": state.satellite_count(),
                "used_in_fix": state.used_satellites(),
                "avg_cn0": state.avg_cn0(),
            },
            "position": {
                "latitude": state.position_lat,
                "longitude": state.position_lon,
                "altitude_m": state.altitude,
            },
            "metrics": {
                "hdop": state.hdop,
                "pdop": state.pdop,
                "velocity_ms": state.velocity,
            },
        });

        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &report)?;
        Ok(())
    }
}

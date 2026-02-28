//! GLOS-UI: интерфейс для GNSS/SDR анализа
//!
//! Модуль обеспечивает визуализацию сигнала, спутников и метаданных
//! в режиме реального времени.

pub mod app;
pub mod data;
pub mod panels;
pub mod theme;

pub use app::GlosApp;
pub use data::{AppState, MockDataGenerator};

/// Запуск UI приложения.
pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("ГЛОС - Инструмент анализа GNSS/SDR"),
        ..Default::default()
    };

    eframe::run_native(
        "ГЛОС",
        options,
        Box::new(|cc| Ok(Box::new(GlosApp::new(cc)))),
    )
}

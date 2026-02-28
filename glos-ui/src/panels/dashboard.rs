use std::sync::Arc;

use egui_plot::{Line, Plot, PlotPoints};
use parking_lot::RwLock;

use crate::AppState;

pub struct Dashboard;

impl Dashboard {
    pub fn render(
        ui: &mut egui::Ui,
        state: &Arc<RwLock<AppState>>,
    ) {
        let state = state.read();

        ui.heading("Панель мониторинга");
        ui.separator();

        // Верхняя строка - основные метрики
        ui.horizontal(|ui| {
            Self::metric_card(ui, "Спутники", &format!("{}", state.satellite_count()), "🛰");
            Self::metric_card(
                ui,
                "Используются в решении",
                &format!("{}", state.used_satellites()),
                "✓",
            );
            Self::metric_card(
                ui,
                "Средний CN0",
                &format!("{:.1} дБГц", state.avg_cn0()),
                "📡",
            );
            Self::metric_card(
                ui,
                "Гор. точность (HDOP)",
                &format!("{:.2}", state.hdop),
                "🎯",
            );
        });

        ui.add_space(10.0);

        // Вторая строка - положение и метрики
        ui.horizontal(|ui| {
            Self::metric_card(
                ui,
                "Положение",
                &format!("{:.4}°N\n{:.4}°E", state.position_lat, state.position_lon),
                "🌍",
            );
            Self::metric_card(ui, "Высота", &format!("{:.1} м", state.altitude), "⛰");
            Self::metric_card(ui, "Скорость", &format!("{:.2} м/с", state.velocity), "💨");
            Self::metric_card(
                ui,
                "Загрузка ЦП",
                &format!("{:.1}%", state.metrics.cpu_usage),
                "💻",
            );
        });

        ui.add_space(20.0);

        // График CN0 во времени
        ui.heading("История CN0");
        let cn0_history: PlotPoints = state
            .cn0_history
            .iter()
            .enumerate()
            .map(|(i, (_, cn0))| [i as f64, *cn0 as f64])
            .collect();

        Plot::new("cn0_plot")
            .height(200.0)
            .show_axes([true, true])
            .show_grid([true, true])
            .allow_zoom(false)
            .allow_drag(false)
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new("CN0", cn0_history)
                        .color(egui::Color32::from_rgb(100, 200, 100))
                        .width(2.0),
                );
            });

        ui.add_space(10.0);

        // Системные метрики
        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label("Системные метрики");
                    ui.separator();
                    ui.label(format!(
                        "Полоса пропускания: {:.1} МГц",
                        state.metrics.bandwidth_mhz
                    ));
                    ui.label(format!("Буфер: {:.1}%", state.metrics.buffer_usage));
                    ui.label(format!("Пакеты/с: {}", state.metrics.packets_per_sec));
                });
            });

            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(format!(
                        "Центральная частота: {:.2} МГц",
                        state.signal_data.frequency_mhz
                    ));
                    ui.label(format!(
                        "Частота дискретизации: {:.1} МГц",
                        state.signal_data.sample_rate_mhz
                    ));
                    ui.label(format!("Размер FFT: {}", state.signal_data.fft_data.len()));
                });
            });
        });
    }

    pub fn metric_card(
        ui: &mut egui::Ui,
        label: &str,
        value: &str,
        icon: &str,
    ) {
        ui.group(|ui| {
            ui.set_min_width(120.0);
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(icon).size(24.0));
                ui.label(egui::RichText::new(label).small());
                ui.label(egui::RichText::new(value).strong());
            });
        });
    }
}

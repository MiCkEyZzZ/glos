use std::{f32, sync::Arc};

use egui::Color32;
use egui_plot::{Line, Plot, PlotPoints};
use parking_lot::RwLock;

use crate::data::AppState;

pub struct SignalPanel;

impl SignalPanel {
    pub fn render(
        ui: &mut egui::Ui,
        state: &Arc<RwLock<AppState>>,
    ) {
        let state = state.read();

        ui.heading("📡 Просмотр сигнала");
        ui.separator();

        // FFT спектр
        ui.label(
            egui::RichText::new(format!(
                "Центральная частота: {:.2} МГц | Частота дискретизации: {:.1} МГц",
                state.signal_data.frequency_mhz, state.signal_data.sample_rate_mhz
            ))
            .strong(),
        );

        ui.add_space(5.0);

        // График FFT
        let fft_points: PlotPoints = state
            .signal_data
            .fft_data
            .iter()
            .enumerate()
            .map(|(i, power)| {
                let freq = (i as f32 / state.signal_data.fft_data.len() as f32 - 0.5)
                    * state.signal_data.sample_rate_mhz
                    + state.signal_data.frequency_mhz;
                [freq as f64, *power as f64]
            })
            .collect();

        Plot::new("fft_plot")
            .height(300.0)
            .show_axes([true, true])
            .show_grid([true, true])
            .allow_zoom(true)
            .allow_drag(true)
            .x_axis_label("Частота (МГц)")
            .y_axis_label("Мощность (дБ)")
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new("FFT", fft_points)
                        .color(egui::Color32::from_rgb(100, 150, 250))
                        .width(1.5),
                );
            });

        ui.add_space(15.0);

        // Waterfall (упрощенная версия)
        ui.heading("Водопад спектра");

        let waterfall_size = state.signal_data.waterfall.len();
        if waterfall_size > 0 {
            ui.label(format!("История: {waterfall_size} кадров"));

            // Рисуем waterfall как серию линий
            Plot::new("waterfall_plot")
                .height(300.0)
                .show_axes([true, true])
                .show_grid([false, false])
                .allow_zoom(true)
                .x_axis_label("Бин частоты")
                .y_axis_label("Время (кадры)")
                .show(ui, |plot_ui| {
                    for (time_idx, row) in state.signal_data.waterfall.iter().enumerate() {
                        let points: PlotPoints = row
                            .iter()
                            .enumerate()
                            .map(|(freq_idx, power)| {
                                // Нормализуем мощность для цвета
                                [freq_idx as f64, time_idx as f64 + (*power as f64) / 20.0]
                            })
                            .collect();

                        let intensity = (time_idx as f32 / waterfall_size as f32 * 255.0) as u8;
                        let color =
                            egui::Color32::from_rgb(intensity / 2, intensity, 255 - intensity / 2);

                        plot_ui.line(
                            Line::new(format!("wf_{time_idx}"), points)
                                .color(color)
                                .width(1.0),
                        );
                    }
                });
        } else {
            ui.label("Данные водопада отсутствуют");
        }

        ui.add_space(10.0);

        // Статистика сигнала
        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label("Статистика сигнала");
                    ui.separator();

                    let max_power = state
                        .signal_data
                        .fft_data
                        .iter()
                        .copied()
                        .fold(f32::NEG_INFINITY, f32::max);
                    let min_power = state
                        .signal_data
                        .fft_data
                        .iter()
                        .copied()
                        .fold(f32::INFINITY, f32::min);
                    let avg_power: f32 = state.signal_data.fft_data.iter().sum::<f32>()
                        / state.signal_data.fft_data.len() as f32;

                    ui.label(format!("Макс: {max_power:.1} дБ"));
                    ui.label(format!("Мин: {min_power:.1} дБ"));
                    ui.label(format!("Среднее: {avg_power:.1} дБ"));
                    ui.label(format!(
                        "Динамический диапазон: {:.1} дБ",
                        max_power - min_power
                    ));
                });
            });
        });
    }

    /// Преобразует мощность (дБ) в цвет (типа Virdis или Jet colormap)
    #[allow(dead_code)]
    fn power_to_color(
        power_db: f32,
        min_db: f32,
        max_db: f32,
    ) -> Color32 {
        let normalized = ((power_db - min_db) / (max_db - min_db)).clamp(0.0, 1.0);

        // Jet-like colormap: синий -> голубой -> зелёный -> жёлтый -> красный
        let (r, g, b) = if normalized < 0.25 {
            let t = normalized / 0.25;
            (0.0, 255.0 * t, 255.0)
        } else if normalized < 0.5 {
            let t = (normalized - 0.25) / 0.25;
            (0.0, 255.0, 255.0 * (1.0 - t))
        } else if normalized < 0.75 {
            let t = (normalized - 0.5) / 0.25;
            (255.0 * t, 255.0, 0.0)
        } else {
            let t = (normalized - 0.75) / 0.25;
            (255.0, 255.0 * (1.0 - t), 0.0)
        };

        Color32::from_rgb(
            r.round().clamp(0.0, 255.0) as u8,
            g.round().clamp(0.0, 255.0) as u8,
            b.round().clamp(0.0, 255.0) as u8,
        )
    }

    /// Рисуем waterfall как текстуру (быстрее чем линии)
    #[allow(dead_code)]
    fn render_waterfall_texture(
        ui: &mut egui::Ui,
        waterfall: &std::collections::VecDeque<Vec<f32>>,
    ) {
        if waterfall.is_empty() {
            return;
        }

        let width: usize = waterfall[0].len();
        let height: usize = waterfall.len();

        // Находим min/max для colormap
        let mut min_power = f32::INFINITY;
        let mut max_power = f32::NEG_INFINITY;

        for row in waterfall {
            for &power in row {
                min_power = min_power.min(power);
                max_power = max_power.max(power);
            }
        }

        // Собираем RGBA-буфер (u8)
        let mut rgba: Vec<u8> = Vec::with_capacity(width * height * 4);
        for row in waterfall.iter() {
            for &power in row.iter() {
                let color = Self::power_to_color(power, min_power, max_power);
                let [r, g, b, a] = color.to_array(); // Color32 -> [u8;4]
                rgba.push(r);
                rgba.push(g);
                rgba.push(b);
                rgba.push(a);
            }
        }

        // Создаём ColorImage через from_rgba_unmultiplied
        let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);

        // Загружаем/обновляем текстуру — лучше уникальное имя, чтобы избежать конфликта
        // при обновлениях
        let texture_id = "waterfall_texture";
        let texture = ui
            .ctx()
            .load_texture(texture_id, color_image, egui::TextureOptions::LINEAR);

        // Показываем
        let available_width = ui.available_width();
        let aspect_ratio = width as f32 / height as f32;
        let display_height = (available_width / aspect_ratio).max(1.0);

        let size_vec = egui::Vec2::new(available_width, display_height);
        ui.add(egui::Image::from_texture((texture.id(), size_vec)));
    }
}

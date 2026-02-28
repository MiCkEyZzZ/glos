#[derive(Clone, Copy, PartialEq)]
pub enum ColormapType {
    Jet,
    Viridis,
    Grayscale,
}

#[derive(Clone)]
pub struct UiSettings {
    // Signal view
    pub fft_window_size: usize,
    pub waterfall_colormap: ColormapType,
    pub show_grid: bool,

    // Satellites
    pub min_cn0_threshold: f32,
    pub show_doppler_arrows: bool,
    pub skyplot_labels: bool,

    // Dashboard
    pub update_rate_ms: u64,
    pub history_length: usize,
}

pub struct SettingsPanel;

impl SettingsPanel {
    pub fn render(
        ui: &mut egui::Ui,
        settings: &mut UiSettings,
    ) {
        ui.heading("⚙️ Настройки");
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.collapsing("📡 Просмотр сигнала", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Размер FFT:");
                    egui::ComboBox::from_id_salt("fft_size")
                        .selected_text(format!("{}", settings.fft_window_size))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut settings.fft_window_size, 256, "256");
                            ui.selectable_value(&mut settings.fft_window_size, 512, "512");
                            ui.selectable_value(&mut settings.fft_window_size, 1024, "1024");
                            ui.selectable_value(&mut settings.fft_window_size, 2048, "2048");
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Цветовая карта:");
                    egui::ComboBox::from_id_salt("colormap")
                        .selected_text(match settings.waterfall_colormap {
                            ColormapType::Jet => "Jet",
                            ColormapType::Viridis => "Viridis",
                            ColormapType::Grayscale => "Оттенки серого",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut settings.waterfall_colormap,
                                ColormapType::Jet,
                                "Jet",
                            );
                            ui.selectable_value(
                                &mut settings.waterfall_colormap,
                                ColormapType::Viridis,
                                "Viridis",
                            );
                            ui.selectable_value(
                                &mut settings.waterfall_colormap,
                                ColormapType::Grayscale,
                                "Оттенки серого",
                            );
                        });
                });

                ui.checkbox(&mut settings.show_grid, "Показывать сетку");
            });

            ui.collapsing("🛰 Спутники", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Мин. CN0 (дБГц):");
                    ui.add(egui::Slider::new(
                        &mut settings.min_cn0_threshold,
                        0.0..=50.0,
                    ));
                });

                ui.checkbox(
                    &mut settings.show_doppler_arrows,
                    "Показывать стрелки допплера",
                );
                ui.checkbox(&mut settings.skyplot_labels, "Метки на небесной диаграмме");
            });

            ui.collapsing("📊 Панель мониторинга", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Частота обновления (мс):");
                    ui.add(egui::Slider::new(&mut settings.update_rate_ms, 10..=500));
                });

                ui.horizontal(|ui| {
                    ui.label("Длина истории:");
                    ui.add(egui::Slider::new(&mut settings.history_length, 60..=600));
                });
            });

            ui.separator();

            if ui.button("🔄 Сбросить по умолчанию").clicked() {
                *settings = UiSettings::default();
            }
        });
    }
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            fft_window_size: 512,
            waterfall_colormap: ColormapType::Jet,
            show_grid: true,
            min_cn0_threshold: 25.0,
            show_doppler_arrows: false,
            skyplot_labels: true,
            update_rate_ms: 50,
            history_length: 300,
        }
    }
}

use std::sync::Arc;

use parking_lot::RwLock;

use crate::{
    data::{AppState, MockDataGenerator},
    panels::{Dashboard, LogsPanel, SatellitesPanel, SignalPanel},
    theme,
};

pub struct GlosApp {
    state: Arc<RwLock<AppState>>,
    mock_generator: MockDataGenerator,
    active_panel: ActivePanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivePanel {
    Dashboard,
    Signal,
    Satellites,
    Logs,
}

impl GlosApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::configure_style(&cc.egui_ctx);

        let state = AppState::new();
        let mock_generator = MockDataGenerator::new(Arc::clone(&state));

        Self {
            state,
            mock_generator,
            active_panel: ActivePanel::Dashboard,
        }
    }

    fn render_top_bar(
        &mut self,
        ctx: &egui::Context,
    ) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🛰 ГЛОС");
                ui.separator();

                // КРИТИЧНО: читаем всё сразу и отпускаем lock!
                let (status_color, status_text, sat_count, avg_cn0, cpu_usage) = {
                    let state = self.state.read();
                    (
                        state.status.color(),
                        state.status.as_str().to_string(),
                        state.satellite_count(),
                        state.avg_cn0(),
                        state.metrics.cpu_usage,
                    )
                }; // lock dropped here!

                ui.label("Статус:");
                ui.colored_label(status_color, format!("● {status_text}"));

                ui.separator();

                // Контролы - БЕЗ активного lock на state!
                if self.mock_generator.is_running() {
                    if ui.button("⏹ Стоп").clicked() {
                        self.mock_generator.stop();
                    }
                } else if ui.button("▶ Запустить генератор").clicked() {
                    self.mock_generator.start();
                }

                ui.separator();

                // Быстрая статистика - используем скопированные данные
                ui.label(format!("Спутники: {sat_count}"));
                ui.label(format!("CN0: {avg_cn0:.1} дБГц"));
                ui.label(format!("ЦП: {cpu_usage:.1}%"));
            });
        });
    }

    fn render_side_panel(
        &mut self,
        ctx: &egui::Context,
    ) {
        egui::SidePanel::left("side_panel")
            .default_width(180.0)
            .show(ctx, |ui| {
                ui.heading("Панели");
                ui.separator();

                ui.selectable_value(
                    &mut self.active_panel,
                    ActivePanel::Dashboard,
                    "📊 Панель мониторинга",
                );
                ui.selectable_value(
                    &mut self.active_panel,
                    ActivePanel::Signal,
                    "📡 Просмотр сигнала",
                );
                ui.selectable_value(
                    &mut self.active_panel,
                    ActivePanel::Satellites,
                    "🛰 Спутники",
                );
                ui.selectable_value(
                    &mut self.active_panel,
                    ActivePanel::Logs,
                    "📜 Журнал событий",
                );

                ui.separator();

                // Читаем конфиг и сразу отпускаем lock
                let (freq, sr, bw) = {
                    let state = self.state.read();
                    (
                        state.signal_data.frequency_mhz,
                        state.signal_data.sample_rate_mhz,
                        state.metrics.bandwidth_mhz,
                    )
                }; // lock dropped here!

                ui.heading("Конфигурация");
                ui.label(format!("Частота: {freq:.2} МГц"));
                ui.label(format!("Частота дискретизации: {sr:.1} МГц"));
                ui.label(format!("Полоса пропускания: {bw:.1} МГц"));
            });
    }
}

impl eframe::App for GlosApp {
    fn update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) {
        // Обновление каждые 50ms
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        self.render_top_bar(ctx);
        self.render_side_panel(ctx);

        egui::CentralPanel::default().show(ctx, |ui| match self.active_panel {
            ActivePanel::Dashboard => {
                Dashboard::render(ui, &self.state);
            }
            ActivePanel::Signal => {
                SignalPanel::render(ui, &self.state);
            }
            ActivePanel::Satellites => {
                SatellitesPanel::render(ui, &self.state);
            }
            ActivePanel::Logs => {
                LogsPanel::render(ui, &self.state);
            }
        });
    }
}

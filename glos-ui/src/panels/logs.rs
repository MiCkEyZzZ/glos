use std::sync::Arc;

use parking_lot::RwLock;

use crate::data::AppState;

pub struct LogsPanel;

impl LogsPanel {
    pub fn render(
        ui: &mut egui::Ui,
        state: &Arc<RwLock<AppState>>,
    ) {
        // флаг очистки, ставим если нажата кнопка — сам write сделаем после drop
        // read-guard
        let mut clear_requested = false;

        // возьмём read-guard под другое имя
        let state_read = state.read();

        ui.heading("📜 Системный журнал");
        ui.separator();

        ui.horizontal(|ui| {
            ui.label(format!(
                "Всего сообщений: {}",
                state_read.log_messages.len()
            ));
            if ui.button("Очистить").clicked() {
                // помечаем, что нужно очистить — реальная очистка ниже, после drop(state_read)
                clear_requested = true;
            }
        });

        ui.add_space(10.0);

        // Скроллируемая область логов — безопасно показываем под read-guard
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                for (timestamp, message) in state_read.log_messages.iter().rev() {
                    ui.horizontal(|ui| {
                        let time_str = timestamp.format("%H:%M:%S%.3f").to_string();
                        ui.label(
                            egui::RichText::new(format!("[{time_str}]"))
                                .color(egui::Color32::from_rgb(150, 150, 150))
                                .monospace(),
                        );

                        // подсветка
                        let color = if message.contains("error") || message.contains("Error") {
                            egui::Color32::from_rgb(255, 100, 100)
                        } else if message.contains("warning") || message.contains("Warning") {
                            egui::Color32::from_rgb(255, 200, 100)
                        } else if message.contains("started") || message.contains("acquired") {
                            egui::Color32::from_rgb(100, 255, 100)
                        } else {
                            egui::Color32::from_rgb(220, 220, 220)
                        };

                        ui.label(egui::RichText::new(message).color(color).monospace());
                    });
                }
            });

        // отпускаем read-guard перед получением write-guard
        drop(state_read);

        if clear_requested {
            let mut state_mut = state.write();
            state_mut.log_messages.clear();
        }
    }
}

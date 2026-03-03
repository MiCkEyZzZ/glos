use std::sync::Arc;

use egui::Color32;
use egui_plot::{Plot, Points};
use parking_lot::RwLock;

use crate::{data::Satellite, AppState};

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    Id,
    Constellation,
    CN0,
    Elevation,
    Azimuth,
    Doppler,
}

pub struct SatellitesPanel;

/// Интерактивная таблица с фильтрацией и сортировкой.
pub struct InteractiveSatelliteTable {
    sort_by: SortColumn,
    sort_ascending: bool,
    filter_constellation: Option<String>,
    filter_min_cn0: f32,
}

impl SatellitesPanel {
    pub fn render(
        ui: &mut egui::Ui,
        state: &Arc<RwLock<AppState>>,
    ) {
        let state = state.read();

        ui.heading("🛰 Спутники");
        ui.separator();

        ui.label(format!(
            "Всего: {} | Используются в решении: {}",
            state.satellite_count(),
            state.used_satellites(),
        ));

        ui.add_space(10.0);

        // Sky Plot (полярная диаграмма)
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width() * 0.6);
                Self::render_table(ui, &state);
            });

            ui.separator();

            // Sky plot
            ui.vertical(|ui| {
                Self::render_sky_plot(ui, &state);
            });
        });
    }

    pub fn render_table(
        ui: &mut egui::Ui,
        state: &AppState,
    ) {
        use egui_extras::{Column, TableBuilder};

        TableBuilder::new(ui)
            .striped(true)
            .column(Column::exact(50.0))
            .column(Column::exact(80.0))
            .column(Column::exact(70.0))
            .column(Column::exact(70.0))
            .column(Column::exact(70.0))
            .column(Column::exact(80.0))
            .column(Column::exact(50.0))
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("ИД");
                });
                header.col(|ui| {
                    ui.strong("Созвездие");
                });
                header.col(|ui| {
                    ui.strong("CN0");
                });
                header.col(|ui| {
                    ui.strong("Высота");
                });
                header.col(|ui| {
                    ui.strong("Азимут");
                });
                header.col(|ui| {
                    ui.strong("Доплер");
                });
                header.col(|ui| {
                    ui.strong("Решение");
                });
            })
            .body(|mut body| {
                for sat in &state.satellites {
                    body.row(18.0, |mut row| {
                        row.col(|ui| {
                            ui.label(&sat.id);
                        });
                        row.col(|ui| {
                            let color = match sat.constellation.as_str() {
                                "GPS" => Color32::from_rgb(100, 150, 255),
                                "ГЛОНАСС" => Color32::from_rgb(255, 100, 100),
                                "Галилео" => Color32::from_rgb(100, 255, 150),
                                "Бэйдоу" => Color32::from_rgb(225, 200, 100),
                                _ => Color32::WHITE,
                            };
                            ui.colored_label(color, &sat.constellation);
                        });
                        row.col(|ui| {
                            let cn0_color = if sat.cn0 > 35.0 {
                                Color32::from_rgb(100, 255, 100)
                            } else if sat.cn0 > 25.0 {
                                Color32::from_rgb(255, 200, 100)
                            } else {
                                Color32::from_rgb(255, 100, 100)
                            };
                            ui.colored_label(cn0_color, format!("{:.1}", sat.cn0));
                        });
                        row.col(|ui| {
                            ui.label(format!("{:.0}°", sat.elevation));
                        });
                        row.col(|ui| {
                            ui.label(format!("{:.0}°", sat.azimuth));
                        });
                        row.col(|ui| {
                            ui.label(format!("{:.0} Гц", sat.doppler));
                        });
                        row.col(|ui| {
                            if sat.used_in_fix {
                                ui.colored_label(Color32::from_rgb(100, 255, 100), "✓");
                            } else {
                                ui.label("-");
                            }
                        });
                    });
                }
            });
    }

    fn render_sky_plot(
        ui: &mut egui::Ui,
        state: &AppState,
    ) {
        ui.heading("Полярная диаграмма");
        ui.label("Высота vs Азимут");

        // Преобразуем данные спутников в полярные координаты для отображения
        Plot::new("sky_plot")
            .width(300.0)
            .height(300.0)
            .data_aspect(1.0)
            .show_axes([false, false])
            .show_grid([true, true])
            .allow_zoom(false)
            .allow_drag(false)
            .show(ui, |plot_ui| {
                // Рисуем круги возвышения
                for elev in [30.0, 60.0, 90.0] {
                    let radius = (90.0 - elev) / 90.0;
                    let circle: Vec<[f64; 2]> = (0..=360)
                        .step_by(5)
                        .map(|deg| {
                            let rad = (deg as f32).to_radians();
                            [(radius * rad.cos()) as f64, (radius * rad.sin()) as f64]
                        })
                        .collect();

                    // <-- передаём имя ("circle_<elev>") и данные
                    plot_ui.line(
                        egui_plot::Line::new(format!("circle_{elev:.0}"), circle)
                            .color(Color32::from_gray(60))
                            .width(1.0),
                    );
                }

                // Рисуем спутники
                for (i, sat) in state.satellites.iter().enumerate() {
                    let radius = (90.0 - sat.elevation) / 90.0;
                    let azimuth_rad = sat.azimuth.to_radians();

                    let x = radius * azimuth_rad.sin();
                    let y = radius * azimuth_rad.cos();

                    let color = match sat.constellation.as_str() {
                        "GPS" => Color32::from_rgb(100, 150, 255),
                        "ГЛОНАСС" => Color32::from_rgb(255, 100, 100),
                        "Галилео" => Color32::from_rgb(100, 255, 150),
                        "Бэйдоу" => Color32::from_rgb(255, 200, 100),
                        _ => Color32::WHITE,
                    };

                    let size = if sat.used_in_fix { 8.0 } else { 4.0 };

                    // Points::new тоже требует имя + данные — даём уникальное имя на спутник
                    plot_ui.points(
                        Points::new(format!("sat_{i}"), vec![[x as f64, y as f64]])
                            .color(color)
                            .radius(size),
                    );
                }
            });

        // Легенда
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            // Кружок
            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 6.0, Color32::from_rgb(100, 150, 255));
            // Текст
            ui.label("GPS");

            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 6.0, Color32::from_rgb(255, 100, 100));
            ui.label("ГЛОНАСС");

            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 6.0, Color32::from_rgb(100, 255, 150));
            ui.label("Галилео");

            let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 6.0, Color32::from_rgb(255, 200, 100));
            ui.label("Бэйдоу");
        });
    }

    #[allow(dead_code)]
    fn render_sky_plot_with_labels(
        ui: &mut egui::Ui,
        state: &AppState,
    ) {
        use egui::{Color32, FontId, Pos2, Stroke};

        ui.heading("Полярная диаграмма");

        let plot_size = 350.0;
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(plot_size, plot_size), egui::Sense::hover());

        let painter = ui.painter();
        let center = rect.center();
        let radius = rect.width() / 2.0 - 20.0;

        // Рисуем круги возвышения (30°, 60°, 90°)
        for elev in [30.0, 60.0, 90.0] {
            let r = radius * (90.0 - elev) / 90.0;
            painter.circle_stroke(center, r, Stroke::new(1.0, Color32::from_gray(60)));

            // Метка высоты
            painter.text(
                Pos2::new(center.x + r + 5.0, center.y),
                egui::Align2::LEFT_CENTER,
                format!("{elev:.0}°"),
                FontId::proportional(10.0),
                Color32::from_gray(120),
            );
        }

        // Рисуем оси N-S-E-W
        painter.line_segment(
            [
                Pos2::new(center.x, center.y - radius),
                Pos2::new(center.x, center.y + radius),
            ],
            Stroke::new(1.0, Color32::from_gray(80)),
        );
        painter.line_segment(
            [
                Pos2::new(center.x - radius, center.y),
                Pos2::new(center.x + radius, center.y),
            ],
            Stroke::new(1.0, Color32::from_gray(80)),
        );

        // Метки направлений
        painter.text(
            center + egui::vec2(0.0, -radius - 10.0),
            egui::Align2::CENTER_CENTER,
            "С",
            FontId::proportional(12.0),
            Color32::WHITE,
        );
        painter.text(
            center + egui::vec2(0.0, radius + 10.0),
            egui::Align2::CENTER_CENTER,
            "Ю",
            FontId::proportional(12.0),
            Color32::WHITE,
        );
        painter.text(
            center + egui::vec2(radius + 10.0, 0.0),
            egui::Align2::CENTER_CENTER,
            "В",
            FontId::proportional(12.0),
            Color32::WHITE,
        );
        painter.text(
            center + egui::vec2(-radius - 10.0, 0.0),
            egui::Align2::CENTER_CENTER,
            "З",
            FontId::proportional(12.0),
            Color32::WHITE,
        );

        // Рисуем спутники
        for sat in &state.satellites {
            let r = radius * (90.0 - sat.elevation) / 90.0;
            let angle_rad = (90.0 - sat.azimuth).to_radians(); // поворот чтобы север был вверху

            let x = center.x + r * angle_rad.cos();
            let y = center.y - r * angle_rad.sin();
            let pos = Pos2::new(x, y);

            let color = match sat.constellation.as_str() {
                "GPS" => Color32::from_rgb(100, 150, 255),
                "ГЛОНАСС" => Color32::from_rgb(255, 100, 100),
                "Галилео" => Color32::from_rgb(100, 255, 150),
                "Бэйдоу" => Color32::from_rgb(255, 200, 100),
                _ => Color32::WHITE,
            };

            let point_radius = if sat.used_in_fix { 6.0 } else { 4.0 };
            painter.circle_filled(pos, point_radius, color);

            // Рисуем ID спутника рядом
            painter.text(
                pos + egui::vec2(8.0, -8.0),
                egui::Align2::LEFT_BOTTOM,
                &sat.id,
                FontId::monospace(10.0),
                color,
            );
        }
    }
}

impl InteractiveSatelliteTable {
    pub fn new() -> Self {
        Self {
            sort_by: SortColumn::CN0,
            sort_ascending: false,
            filter_constellation: None,
            filter_min_cn0: 0.0,
        }
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        satellites: &[Satellite],
    ) {
        // Фильтры
        ui.horizontal(|ui| {
            ui.label("Фильтр:");

            egui::ComboBox::from_label("Созвездие")
                .selected_text(self.filter_constellation.as_deref().unwrap_or("Все"))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter_constellation, None, "Все");
                    ui.selectable_value(
                        &mut self.filter_constellation,
                        Some("GPS".to_string()),
                        "GPS",
                    );
                    ui.selectable_value(
                        &mut self.filter_constellation,
                        Some("ГЛОНАСС".to_string()),
                        "ГЛОНАСС",
                    );
                    ui.selectable_value(
                        &mut self.filter_constellation,
                        Some("Галилео".to_string()),
                        "Галилео",
                    );
                    ui.selectable_value(
                        &mut self.filter_constellation,
                        Some("Бэйдоу".to_string()),
                        "Бэйдоу",
                    );
                });

            ui.label("Мин. CN0:");
            ui.add(egui::Slider::new(&mut self.filter_min_cn0, 0.0..=50.0).suffix(" дБГц"));
        });

        ui.separator();

        // Применяем фильтры
        let mut filtered: Vec<_> = satellites
            .iter()
            .filter(|sat| {
                if self
                    .filter_constellation
                    .as_deref()
                    .is_some_and(|cf| cf != sat.constellation.as_str())
                {
                    return false;
                }
                sat.cn0 >= self.filter_min_cn0
            })
            .cloned()
            .collect();

        // Сортировка
        filtered.sort_by(|a, b| {
            let cmp = match self.sort_by {
                SortColumn::Id => a.id.cmp(&b.id),
                SortColumn::Constellation => a.constellation.cmp(&b.constellation),
                SortColumn::CN0 => a.cn0.partial_cmp(&b.cn0).unwrap(),
                SortColumn::Elevation => a.elevation.partial_cmp(&b.elevation).unwrap(),
                SortColumn::Azimuth => a.azimuth.partial_cmp(&b.azimuth).unwrap(),
                SortColumn::Doppler => a.doppler.partial_cmp(&b.doppler).unwrap(),
            };

            if self.sort_ascending {
                cmp
            } else {
                cmp.reverse()
            }
        });

        // Таблица с кликабельными заголовками
        use egui_extras::{Column, TableBuilder};

        TableBuilder::new(ui)
            .striped(true)
            .column(Column::exact(50.0))
            .column(Column::exact(80.0))
            .column(Column::exact(70.0))
            .column(Column::exact(70.0))
            .column(Column::exact(70.0))
            .column(Column::exact(80.0))
            .column(Column::exact(50.0))
            .header(20.0, |mut header| {
                header.col(|ui| {
                    if ui.button("ИД ▼").clicked() {
                        self.toggle_sort(SortColumn::Id);
                    }
                });
                header.col(|ui| {
                    if ui.button("Созвездие ▼").clicked() {
                        self.toggle_sort(SortColumn::Constellation);
                    }
                });
                header.col(|ui| {
                    if ui.button("CN0 ▼").clicked() {
                        self.toggle_sort(SortColumn::CN0);
                    }
                });
                header.col(|ui| {
                    if ui.button("Высота ▼").clicked() {
                        self.toggle_sort(SortColumn::Elevation);
                    }
                });
                header.col(|ui| {
                    if ui.button("Азимут ▼").clicked() {
                        self.toggle_sort(SortColumn::Azimuth);
                    }
                });
                header.col(|ui| {
                    if ui.button("Доплер ▼").clicked() {
                        self.toggle_sort(SortColumn::Doppler);
                    }
                });
                header.col(|ui| {
                    ui.strong("Решение");
                });
            })
            .body(|mut body| {
                for _sat in &filtered {
                    body.row(18.0, |_row| {
                        // ... (рендер строк как раньше)
                    });
                }
            });

        ui.label(format!(
            "Показано: {} из {}",
            filtered.len(),
            satellites.len()
        ));
    }

    fn toggle_sort(
        &mut self,
        column: SortColumn,
    ) {
        if self.sort_by == column {
            self.sort_ascending = !self.sort_ascending;
        } else {
            self.sort_by = column;
            self.sort_ascending = false;
        }
    }
}

impl Default for InteractiveSatelliteTable {
    fn default() -> Self {
        Self::new()
    }
}

use egui::{Color32, Context, Stroke, Style, Visuals};

pub fn configure_style(ctx: &Context) {
    let mut style = Style::default();
    let mut visuals = Visuals::dark();

    // Цветовая схема - тёмная, технарская
    visuals.window_fill = Color32::from_rgb(25, 28, 32);
    visuals.panel_fill = Color32::from_rgb(30, 33, 38);
    visuals.faint_bg_color = Color32::from_rgb(40, 44, 50);

    visuals.extreme_bg_color = Color32::from_rgb(15, 18, 22);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(60, 65, 72));

    // Текст — через override_text_color / weak_text_color (Option<Color32>)
    visuals.override_text_color = Some(Color32::from_rgb(220, 220, 220));
    visuals.weak_text_color = Some(Color32::from_rgb(150, 150, 150));
    visuals.hyperlink_color = Color32::from_rgb(100, 150, 255);

    // Акценты (selection — структура с полями bg_fill и stroke)
    visuals.selection.bg_fill = Color32::from_rgb(60, 100, 180);
    visuals.selection.stroke = Stroke::new(1.0, Color32::from_rgb(100, 150, 255));

    // Виджеты: цвета и stroke (поля доступны в 0.33)
    visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(40, 44, 50);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(100, 105, 110));

    visuals.widgets.inactive.bg_fill = Color32::from_rgb(50, 54, 60);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(120, 125, 130));

    visuals.widgets.hovered.bg_fill = Color32::from_rgb(60, 70, 80);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, Color32::from_rgb(140, 150, 160));

    visuals.widgets.active.bg_fill = Color32::from_rgb(70, 100, 140);
    visuals.widgets.active.fg_stroke = Stroke::new(2.0, Color32::from_rgb(100, 150, 255));

    // ВНИМАНИЕ: в egui 0.33 поле window_rounding у Visuals может отсутствовать.
    // Если тебе обязательно нужно менять скругления виджетов/окон — см. пояснение
    // ниже.

    style.visuals = visuals;

    // Отступы
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(10);

    ctx.set_style(style);
}

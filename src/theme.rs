use eframe::egui;

use egui::{style, Color32};

pub fn apply_theme(ctx: &egui::Context) {
    let previous_theme = ctx.style().visuals.clone();

    let accent = Color32::from_rgb(220, 220, 220);
    let bg = Color32::from_rgb(48, 48, 48);
    let wbg = Color32::from_rgb(200, 200, 200);
    let light_bg = Color32::from_rgb(150, 150, 150);
    let font = Color32::from_rgb(185, 185, 185);

    ctx.set_visuals(egui::Visuals {
        override_text_color: Some(font),
        window_fill: bg,
        panel_fill: bg,
        button_frame: true,
        extreme_bg_color: Color32::from_rgb(127, 127, 127),
        widgets: style::Widgets {
            noninteractive: create_widget_visuals(
                previous_theme.widgets.noninteractive,
                wbg,
                accent,
            ),
            inactive: create_widget_visuals(previous_theme.widgets.hovered, wbg, bg),
            hovered: create_widget_visuals(previous_theme.widgets.hovered, light_bg, bg),
            active: create_widget_visuals(previous_theme.widgets.active, wbg, light_bg),
            open: create_widget_visuals(previous_theme.widgets.open, wbg, bg),
        },
        ..previous_theme
    });
}

fn create_widget_visuals(
    previous: style::WidgetVisuals,
    bg_fill: egui::Color32,
    stroke: egui::Color32,
) -> style::WidgetVisuals {
    style::WidgetVisuals {
        bg_fill,
        bg_stroke: egui::Stroke {
            color: stroke,
            ..previous.bg_stroke
        },
        ..previous
    }
}

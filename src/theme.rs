use eframe::egui;

use egui::{style, Color32};
use epaint::{
    text::{FontData, FontDefinitions},
    FontFamily,
};

pub fn apply_theme(ctx: &egui::Context) {
    #[cfg(feature = "custom_font")]
    apply_fonts(ctx);

    let previous_theme = ctx.style().visuals.clone();

    let accent = Color32::from_rgb(220, 220, 220);
    let bg = Color32::from_rgb(48, 48, 48);
    let wbg = Color32::from_rgb(200, 200, 200);
    let extreme_bg = Color32::from_rgb(70, 70, 70);
    let light_bg = Color32::from_rgb(150, 150, 150);
    let font = Color32::from_rgb(185, 185, 185);

    ctx.set_visuals(egui::Visuals {
        override_text_color: Some(font),
        window_fill: bg,
        panel_fill: bg,
        button_frame: true,
        extreme_bg_color: extreme_bg,
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

#[cfg(feature = "custom_font")]
pub fn apply_fonts(ctx: &egui::Context) {
    tracing::info!("Applying custom fonts");

    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "custom_font".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../resources/Atkinson_Hyperlegible_Next/AtkinsonHyperlegibleNext-Regular.ttf"
        ))),
    );

    fonts.font_data.insert(
        "custom_font_italic".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../resources/Atkinson_Hyperlegible_Next/AtkinsonHyperlegibleNext-Italic.ttf"
        ))),
    );

    let mut_fonts = fonts.families.get_mut(&FontFamily::Proportional).unwrap();

    mut_fonts.insert(0, "custom_font".to_owned());
    mut_fonts.insert(1, "custom_font_italic".to_owned());

    ctx.set_fonts(fonts);
}

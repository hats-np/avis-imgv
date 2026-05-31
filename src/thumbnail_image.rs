use crate::app::AppState;
use crate::image_store::ImageStore;
use crate::utils::format_stars_from_rating;
use crate::{COLOR_GREY_DARK_BG, COLOR_GREY_DARK_TRANSPARENT_BG, COLOR_MID_GREY, COLOR_MID_LIGHT};
use eframe::egui::load::SizedTexture;
use eframe::egui::{self, Response, RichText, UiBuilder, Vec2};
use eframe::epaint::vec2;
use epaint::Rect;
use std::path::PathBuf;

pub struct ThumbnailImage {
    pub path: PathBuf,
    pub name: String,
    pub registered: bool,
    pub display_metadata: Option<Vec<(String, String)>>,
}

impl ThumbnailImage {
    pub fn from_paths(paths: &[PathBuf]) -> Vec<Self> {
        paths
            .iter()
            .map(|p| Self {
                path: p.clone(),
                name: p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                registered: false,
                display_metadata: None,
            })
            .collect()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        mut size: [f32; 2],
        state: &mut AppState,
        metadata_tags_to_show: &[String],
        image_selected: bool,
    ) -> Option<Response> {
        if !state.thumbnail_store.is_image_loaded(&self.path) {
            Self::display_empty_image_frame(ui, size[1]);
            return None;
        }

        let image_size = match state.thumbnail_store.get_image_size(&self.path) {
            Some(size) => size,
            None => {
                Self::display_empty_image_frame(ui, size[1]);
                return None;
            }
        };

        let texture_id = match state.thumbnail_store.get_texture_id(&self.path) {
            Some(texture_id) => texture_id,
            None => {
                Self::display_empty_image_frame(ui, size[1]);
                return None;
            }
        };

        let prev_size = [size[0], size[1]];
        let aspect_ratio = image_size.x / image_size.y;

        if aspect_ratio > 1. {
            size[1] /= aspect_ratio;
        } else {
            size[0] *= aspect_ratio;
        }

        let mut response: Option<Response> = None;
        let rect_size = Vec2::splat(prev_size[1]);
        let rect = ui.allocate_space(rect_size);

        let bg_color = if image_selected {
            COLOR_MID_LIGHT
        } else {
            COLOR_MID_GREY
        };

        ui.painter().rect_filled(rect.1, 0, bg_color);

        ui.scope_builder(UiBuilder::new().max_rect(rect.1), |ui| {
            ui.centered_and_justified(|ui| {
                let img_response = ui.add(
                    egui::Image::new(SizedTexture::new(texture_id, image_size))
                        .fit_to_exact_size(vec2(size[0], size[1]))
                        .sense(egui::Sense::CLICK),
                );

                response = Some(img_response)
            });
        });

        if state.view_visibility.side_panel {
            self.metadata_strip_ui(ui, &mut state.thumbnail_store, rect.1);
        }

        if state.grouping.enabled
            && state.view_visibility.side_panel
            && let Some(paths) = state.get_grouped_img_paths(&self.path, true)
        {
            self.group_img_count_ui(ui, &paths, rect.1);
        }

        response.clone().unwrap().on_hover_ui(|ui| {
            self.metadata_ui(
                ui,
                &mut state.thumbnail_store,
                metadata_tags_to_show,
                rect.1,
            );
        });

        let (stroke_width, stroke_color, stroke_kind) = if image_selected {
            (
                2.0,
                state.general_config.accent_color,
                egui::StrokeKind::Inside,
            ) 
        } else {
            (1.0, COLOR_GREY_DARK_BG, egui::StrokeKind::Outside)
        };

        ui.painter().rect_stroke(
            rect.1,
            0.0,
            egui::Stroke::new(stroke_width, stroke_color),
            stroke_kind,
        );

        response
    }

    pub fn display_empty_image_frame(ui: &mut egui::Ui, size: f32) {
        let rect_size = Vec2::splat(size);
        let rect = ui.allocate_space(rect_size);

        ui.painter().rect_filled(rect.1, 0, COLOR_MID_GREY);

        ui.scope_builder(UiBuilder::new().max_rect(rect.1), |ui| {
            ui.centered_and_justified(|ui| {
                let spinner_size = size / 3.;
                ui.add(egui::Spinner::new().size(spinner_size));
            });
        });

        ui.painter().rect_stroke(
            rect.1,
            0.,
            egui::Stroke::new(1.0, COLOR_GREY_DARK_BG),
            egui::StrokeKind::Outside,
        );
    }

    pub fn group_img_count_ui(&mut self, ui: &mut egui::Ui, paths: &[PathBuf], rect: Rect) {
        let overlay_size = 22.0;
        let margin = 4.0;

        let overlay_rect = egui::Rect::from_min_max(
            egui::pos2(rect.right() - overlay_size - margin, rect.top() + margin),
            egui::pos2(rect.right() - margin, rect.top() + overlay_size + margin),
        );

        ui.painter()
            .rect_filled(overlay_rect, 4.0, COLOR_GREY_DARK_TRANSPARENT_BG);

        let mut overlay_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(overlay_rect)
                .layout(egui::Layout::default()),
        );

        overlay_ui.centered_and_justified(|ui| {
            let r = ui.add(
                egui::Label::new(paths.len().to_string()).wrap_mode(egui::TextWrapMode::Truncate),
            );

            if r.hovered() {
                ui.set_cursor_icon(egui::CursorIcon::PointingHand);
                r.clone().on_hover_ui(|ui| {
                    let mut it = paths.iter();

                    if let Some(first) = it.next() {
                        ui.label(egui::RichText::new(first.to_string_lossy()).strong());
                    }

                    for p in it {
                        ui.label(p.to_string_lossy());
                    }
                });
            }

            r
        });
    }

    pub fn metadata_strip_ui(
        &mut self,
        ui: &mut egui::Ui,
        image_store: &mut ImageStore,
        rect: Rect,
    ) {
        if let Some(metadata) = image_store.get_image_metadata(&self.path) {
            if metadata.rating.is_none() {
                return;
            }

            let strip_height = 24.0;
            let strip_rect = egui::Rect::from_min_max(
                egui::pos2(rect.min.x, rect.max.y - strip_height),
                rect.max,
            );

            ui.painter()
                .rect_filled(strip_rect, 0.0, COLOR_GREY_DARK_TRANSPARENT_BG);

            ui.scope_builder(UiBuilder::new().max_rect(strip_rect), |ui| {
                ui.centered_and_justified(|ui| {
                    ui.add(
                        egui::Label::new(format_stars_from_rating(metadata.rating))
                            .wrap_mode(egui::TextWrapMode::Truncate),
                    );
                });
            });
        }
    }

    pub fn metadata_ui(
        &mut self,
        ui: &mut egui::Ui,
        image_store: &mut ImageStore,
        metadata_tags_to_show: &[String],
        _rect: Rect,
    ) {
        if let Some(metadata) = image_store.get_image_metadata(&self.path) {
            if self.display_metadata.is_none() {
                let mut img_metadata: Vec<(String, String)> = vec![];
                for tag in metadata_tags_to_show {
                    if metadata.exif_tags.contains_key(tag) {
                        img_metadata.push((tag.to_string(), metadata.exif_tags[tag].to_string()));
                    }
                }

                self.display_metadata = Some(img_metadata);
            }

            ui.horizontal(|ui| {
                ui.label(RichText::new("Rating: ").strong());
                ui.label(format_stars_from_rating(metadata.rating))
            });

            if let Some(metadata) = &self.display_metadata {
                for md in metadata {
                    ui.horizontal(|ui| {
                        let text = RichText::new(format!("{}:", md.0)).strong();
                        ui.label(text);
                        ui.label(&md.1);
                    });
                }
            }
        }
    }
}

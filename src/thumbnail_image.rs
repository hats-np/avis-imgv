use crate::image_store::ImageStore;
use eframe::egui::load::SizedTexture;
use eframe::egui::{self, Color32, Response, RichText, UiBuilder, Vec2};
use eframe::epaint::vec2;
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
                display_metadata: None
            })
            .collect()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        mut size: [f32; 2],
        image_store: &mut ImageStore,
        metadata_tags_to_show: &[String],
    ) -> Option<Response> {
        if !image_store.is_image_loaded(&self.path) {
            Self::display_empty_image_frame(ui, size[1]);
            return None;
        }

        let image_size = match image_store.get_image_size(&self.path) {
            Some(size) => size,
            None => {
                Self::display_empty_image_frame(ui, size[1]);
                return None;
            }
        };

        let texture_id = match image_store.get_texture_id(&self.path) {
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

        ui.painter()
            .rect_filled(rect.1, 0, egui::Color32::from_rgb(119, 119, 119));

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

        response.clone().unwrap().on_hover_ui(|ui| {
            self.metadata_ui(ui, image_store, metadata_tags_to_show);
        });

        ui.painter().rect_stroke(
            rect.1,
            0., // Corner rounding (must match the one in `rect_filled`)
            egui::Stroke::new(1.0, Color32::from_rgb(48, 48, 48)),
            egui::StrokeKind::Outside, // Border thickness and color
        );

        response
    }

    pub fn display_empty_image_frame(ui: &mut egui::Ui, size: f32) {
        let rect_size = Vec2::splat(size);
        let rect = ui.allocate_space(rect_size);

        ui.painter()
            .rect_filled(rect.1, 0, egui::Color32::from_rgb(119, 119, 119));

        ui.scope_builder(UiBuilder::new().max_rect(rect.1), |ui| {
            ui.centered_and_justified(|ui| {
                let spinner_size = size / 3.;
                ui.add(egui::Spinner::new().size(spinner_size));
            });
        });

        ui.painter().rect_stroke(
            rect.1,
            0., // Corner rounding (must match the one in `rect_filled`)
            egui::Stroke::new(1.0, Color32::from_rgb(48, 48, 48)),
            egui::StrokeKind::Outside, // Border thickness and color
        );
    }

    pub fn metadata_ui(
        &mut self,
        ui: &mut egui::Ui,
        image_store: &mut ImageStore,
        metadata_tags_to_show: &[String],
    ) {
        if self.display_metadata.is_none() {
            let mut img_metadata: Vec<(String, String)> = vec![];
            if let Some(metadata) = image_store.get_image_metadata(&self.path) {
                for tag in metadata_tags_to_show {
                    if metadata.contains_key(tag) {
                        img_metadata.push((tag.to_string(), metadata[tag].to_string()));
                    }
                }
            }
            self.display_metadata = Some(img_metadata);
        }

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

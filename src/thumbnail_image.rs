use crate::image::Image;
use eframe::egui::{self, Color32, Response, UiBuilder, Vec2};
use eframe::epaint::vec2;
use std::path::PathBuf;
use std::thread::JoinHandle;

pub struct ThumbnailImage {
    pub path: PathBuf,
    pub name: String,
    should_unload: bool,
    image: Option<Image>,
    load_image_handle: Option<JoinHandle<Option<Image>>>,
    output_profile: String,
}

impl ThumbnailImage {
    pub fn from_paths(paths: &[PathBuf], output_profile: &String) -> Vec<Self> {
        paths
            .iter()
            .map(|p| Self {
                path: p.clone(),
                name: p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                image: None,
                load_image_handle: None,
                should_unload: false,
                output_profile: output_profile.to_owned(),
            })
            .collect()
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, mut size: [f32; 2]) -> Option<Response> {
        self.finish_img_loading();

        let image = match &mut self.image {
            Some(image) => image,
            None => {
                Self::display_empty_image_frame(ui, size[1]);
                return None;
            }
        };

        let texture = match image.get_texture(&self.name, ui) {
            Some(t) => t,
            None => {
                Self::display_empty_image_frame(ui, size[1]);
                return None;
            }
        };

        let prev_size = [size[0], size[1]];

        if texture.aspect_ratio() > 1. {
            size[1] /= texture.aspect_ratio();
        } else {
            size[0] *= texture.aspect_ratio();
        }

        let mut response: Option<Response> = None;
        let rect_size = Vec2::splat(prev_size[1]);
        let rect = ui.allocate_space(rect_size);

        ui.painter()
            .rect_filled(rect.1, 0, egui::Color32::from_rgb(119, 119, 119));

        ui.allocate_new_ui(UiBuilder::new().max_rect(rect.1), |ui| {
            ui.centered_and_justified(|ui| {
                let img_response = ui
                    .add(
                        egui::Image::new(texture)
                            .fit_to_exact_size(vec2(size[0], size[1]))
                            .sense(egui::Sense::CLICK),
                    )
                    .on_hover_text_at_pointer(&self.name);

                response = Some(img_response)
            });
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

        ui.allocate_new_ui(UiBuilder::new().max_rect(rect.1), |ui| {
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

    pub fn finish_img_loading(&mut self) {
        if self.load_image_handle.is_none() {
            return;
        };

        let lih = self.load_image_handle.take().unwrap();
        if lih.is_finished() {
            match lih.join() {
                Ok(image) => self.image = image,
                Err(_) => println!("Failure joining load image thread"),
            }
        } else {
            self.load_image_handle = Some(lih);
        }
    }

    pub fn load(&mut self, image_size: u32) -> bool {
        if self.load_image_handle.is_none() && self.image.is_none() {
            println!("Loading image -> {}", self.path.display());
            self.should_unload = false;
            self.load_image_handle = Some(Image::load(
                self.path.clone(),
                Some(image_size),
                self.output_profile.clone(),
            ));
            true
        } else {
            false
        }
    }

    ///If image is marked for unloading, unload it
    pub fn unload_delayed(&mut self) {
        if self.should_unload {
            if let Some(ih) = &mut self.load_image_handle {
                if ih.is_finished() {
                    self.load_image_handle = None;
                    self.image = None;
                    self.should_unload = false;
                }
            }
        }
    }

    ///If image is currently loading marks it for unload
    ///If image is loaded, unloads it
    pub fn unload(&mut self, image_nr: usize) {
        if self.load_image_handle.is_some() {
            self.should_unload = true;
            println!(
                "Marking Image Handle for delayed unload {} - {}",
                image_nr, self.name
            );
        } else {
            self.image = None;
            self.load_image_handle = None;
        }
    }

    pub fn is_loading(&self) -> bool {
        match &self.load_image_handle {
            Some(lih) => !lih.is_finished(),
            None => false,
        }
    }
}

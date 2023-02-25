use crate::image::Image;
use eframe::egui::{self, Response};
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
    pub fn from_paths(paths: &Vec<PathBuf>, output_profile: &String) -> Vec<Self> {
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

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        mut size: [f32; 2],
        margin_size: &f32,
    ) -> Option<Response> {
        let mut outer_margin_size = *margin_size;
        size[1] -= outer_margin_size;
        size[0] -= outer_margin_size;
        outer_margin_size = outer_margin_size / 2.;

        self.finish_img_loading();

        let image = match &mut self.image {
            Some(image) => image,
            None => {
                Self::display_empty_image_frame(ui, size[1], outer_margin_size);
                return None;
            }
        };

        let texture = match image.get_texture(&self.name, ui) {
            Some(t) => t,
            None => {
                Self::display_empty_image_frame(ui, size[1], outer_margin_size);
                return None;
            }
        };

        let outer_margin = egui::style::Margin {
            left: outer_margin_size,
            right: outer_margin_size,
            top: outer_margin_size,
            bottom: outer_margin_size,
        };

        let mut margin = egui::style::Margin {
            left: 0.,
            right: 0.,
            top: 0.,
            bottom: 0.,
        };

        let prev = [size[0], size[1]];

        if texture.aspect_ratio() > 1. {
            size[1] = size[1] / texture.aspect_ratio();
            let half_free_y = (prev[1] - size[1]) / 2.;
            margin.top = half_free_y;
            margin.bottom = half_free_y;
        } else {
            size[0] = size[0] * texture.aspect_ratio();
            let half_free_x = (prev[0] - size[0]) / 2.;
            margin.right = half_free_x;
            margin.left = half_free_x;
        }

        let mut response: Option<Response> = None;
        egui::Frame::none()
            .inner_margin(margin)
            .outer_margin(outer_margin)
            .fill(egui::Color32::from_rgb(119, 119, 119))
            .show(ui, |ui| {
                response = Some(ui.add(egui::Image::new(texture, [size[0], size[1]]).sense(
                    egui::Sense {
                        click: (true),
                        drag: (true),
                        focusable: (true),
                    },
                )));
            });

        response
    }

    pub fn display_empty_image_frame(ui: &mut egui::Ui, size: f32, outer_margin: f32) {
        let spinner_size = size / 3.;
        let inner_margin = (size - spinner_size) / 2.;

        egui::Frame::none()
            .inner_margin(egui::style::Margin {
                left: inner_margin,
                right: inner_margin,
                top: inner_margin,
                bottom: inner_margin,
            })
            .outer_margin(egui::style::Margin {
                left: outer_margin,
                right: outer_margin,
                top: outer_margin,
                bottom: outer_margin,
            })
            .fill(egui::Color32::from_rgb(119, 119, 119))
            .show(ui, |ui| ui.add(egui::Spinner::new().size(spinner_size)));
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
        if !self.load_image_handle.is_some() && !self.image.is_some() {
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

    pub fn unload_delayed(&mut self) {
        if self.should_unload {
            match &mut self.load_image_handle {
                Some(ih) => {
                    if ih.is_finished() {
                        self.load_image_handle = None;
                        self.image = None;
                        self.should_unload = false;
                    }
                }
                None => {}
            }
        }
    }

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

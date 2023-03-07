use crate::image::Image;
use eframe::egui::{self, RichText};
use eframe::epaint::{Pos2, Vec2};
use std;
use std::path::PathBuf;
use std::thread::JoinHandle;

pub struct GalleryImage {
    pub path: PathBuf,
    pub name: String,
    scroll_pos: Pos2,
    should_wait: bool,
    image: Option<Image>,
    load_image_handle: Option<JoinHandle<Option<Image>>>,
    output_profile: String,
    display_metadata: Option<Vec<(String, String)>>,
}

impl GalleryImage {
    pub fn from_paths(
        paths: &Vec<PathBuf>,
        should_wait: bool,
        //add a lifetime in the future.
        output_profile: &String,
    ) -> Vec<Self> {
        paths
            .iter()
            .map(|p| Self {
                path: p.clone(),
                name: p
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                scroll_pos: Pos2::new(0.0, 0.0),
                image: None,
                load_image_handle: None,
                should_wait,
                output_profile: output_profile.to_owned(),
                display_metadata: None,
            })
            .collect()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        zoom_factor: &f32,
        scroll_delta: &mut Vec2,
        frame: &bool,
        frame_size_r: &f32,
    ) {
        self.finish_img_loading();

        let image = match &mut self.image {
            Some(image) => image,
            None => {
                Self::display_loading_frame(ui);
                return;
            }
        };

        let texture = match image.get_texture(&self.name, ui) {
            Some(t) => t,
            None => {
                Self::display_loading_frame(ui);
                return;
            }
        };

        let mut size = texture.size_vec2();
        let aspect_ratio = texture.aspect_ratio();

        if ui.available_width() < size[0] {
            size[0] = ui.available_width();
            size[1] = size[0] / aspect_ratio;
        }

        if ui.available_height() < size[1] {
            size[1] = ui.available_height();
            size[0] = aspect_ratio * size[1];
        }

        let mut display_size = size.clone();
        size[0] *= zoom_factor;
        size[1] *= zoom_factor;

        display_size[0] *= zoom_factor;
        if display_size[0] > ui.available_width() {
            display_size[0] = ui.available_width();
        }

        display_size[1] *= zoom_factor;
        if display_size[1] > ui.available_height() {
            display_size[1] = ui.available_height();
        }

        let mut visible_rect = egui::Rect {
            min: egui::Pos2 { x: 0.0, y: 0.0 },
            max: egui::Pos2 { x: 1.0, y: 1.0 },
        };

        let out_bounds_y = size[1] - display_size[1];
        if out_bounds_y > 0.0 {
            let rect_offset_y = (1.0 - (display_size[1] / size[1])) / 2.0;
            visible_rect.min.y = rect_offset_y;
            visible_rect.max.y = 1.0 - rect_offset_y;
        }

        let out_bounds_x = size[0] - display_size[0];
        if out_bounds_x > 0.0 {
            let rect_offset_x = (1.0 - (display_size[0] / size[0])) / 2.0;
            visible_rect.min.x = rect_offset_x;
            visible_rect.max.x = 1.0 - rect_offset_x;
        }

        let free_space = Pos2::new(
            1.0 - (visible_rect.max.x - visible_rect.min.x),
            1.0 - (visible_rect.max.y - visible_rect.min.y),
        );

        Self::update_scroll_pos(&mut self.scroll_pos, free_space, scroll_delta);

        visible_rect.min.y += self.scroll_pos.y / 2.0;
        visible_rect.max.y += self.scroll_pos.y / 2.0;
        visible_rect.min.x += self.scroll_pos.x / 2.0;
        visible_rect.max.x += self.scroll_pos.x / 2.0;

        if *frame {
            //we use the shortest side
            let stroke = if display_size[0] > display_size[1] {
                display_size[1] * frame_size_r
            } else {
                display_size[0] * frame_size_r
            };

            let aspect_ratio = display_size[0] / display_size[1];

            display_size[0] = display_size[0] - stroke;
            display_size[1] = display_size[1] - (stroke / aspect_ratio);

            let image = egui::Image::new(texture, display_size).uv(visible_rect);

            let available_width_per_h_side = (ui.available_width() - display_size[0]) / 2.;
            let available_width_per_v_side = (ui.available_height() - display_size[1]) / 2.;

            egui::Frame::none()
                .outer_margin(egui::style::Margin {
                    left: available_width_per_h_side,
                    right: available_width_per_h_side,
                    top: available_width_per_v_side,
                    bottom: available_width_per_v_side,
                })
                .stroke(egui::Stroke {
                    color: egui::Color32::WHITE,
                    width: stroke,
                })
                .show(ui, |ui| {
                    ui.add(image);
                });
        } else {
            ui.add(egui::Image::new(texture, [display_size[0], display_size[1]]).uv(visible_rect));
        }
    }

    fn update_scroll_pos(
        scroll_pos: &mut Pos2,
        free_space: Pos2,
        scroll_delta: &mut eframe::egui::Vec2,
    ) {
        //reverse scroll directions
        scroll_delta.x *= -1.0;
        scroll_delta.y *= -1.0;

        if free_space.x != 0.0 {
            //has available space to scroll in the x direction
            scroll_pos.x += scroll_delta.x * 0.0015;
            if scroll_pos.x > free_space.x {
                scroll_pos.x = free_space.x;
            } else if scroll_pos.x < free_space.x * -1.0 {
                scroll_pos.x = free_space.x * -1.0;
            }
        } else {
            scroll_pos.x = 0.0;
        }

        if free_space.y != 0.0 {
            //has available space to scroll in the y direction
            scroll_pos.y += scroll_delta.y * 0.0015;
            if scroll_pos.y > free_space.y {
                scroll_pos.y = free_space.y;
            } else if scroll_pos.y < free_space.y * -1.0 {
                scroll_pos.y = free_space.y * -1.0;
            }
        } else {
            scroll_pos.y = 0.0;
        }
    }

    pub fn finish_img_loading(&mut self) {
        if self.load_image_handle.is_none() {
            return;
        };

        //not ideal can't match because of problem case #3 in https://rust-lang.github.io/rfcs/2094-nll.html
        let lih = self.load_image_handle.take().unwrap();
        if self.should_wait {
            match lih.join() {
                Ok(image) => self.image = image,
                Err(_) => println!("Failure joining load image thread"),
            }
        } else {
            if lih.is_finished() {
                match lih.join() {
                    Ok(image) => self.image = image,
                    Err(_) => println!("Failure joining load image thread"),
                }
            } else {
                self.load_image_handle = Some(lih);
            }
        }
    }

    pub fn metadata_ui(&mut self, ui: &mut egui::Ui, tags_to_display: &Vec<String>) {
        match &mut self.image {
            Some(img) => {
                if self.display_metadata.is_none() {
                    let mut metadata: Vec<(String, String)> = vec![];
                    for tag in tags_to_display {
                        match &img.metadata.get(tag) {
                            Some(value) => metadata.push((tag.to_string(), value.to_string())),
                            None => {}
                        };
                    }
                    self.display_metadata = Some(metadata);
                }

                if let Some(metadata) = &self.display_metadata {
                    for md in metadata {
                        ui.horizontal_wrapped(|ui| {
                            let text = RichText::new(format!("{}:", md.0)).strong();
                            ui.label(text);
                            ui.label(&md.1);
                        });
                    }
                }
            }
            None => {}
        }
    }

    pub fn unload(&mut self) {
        println!("Unloading image -> {}", self.path.display());
        self.image = None;
        self.load_image_handle = None;
    }

    pub fn load(&mut self) {
        if !self.load_image_handle.is_some() {
            println!("Loading image -> {}", self.path.display());
            self.load_image_handle = Some(Image::load(
                self.path.clone(),
                None,
                self.output_profile.clone(),
            ));
        }
    }

    pub fn display_loading_frame(ui: &mut egui::Ui) {
        let available_w = ui.available_width();
        let available_h = ui.available_height();
        let spinner_size = ui.available_height() / 3.;
        let inner_margin_y = (available_h - spinner_size) / 2.;
        let inner_margin_x = (available_w - spinner_size) / 2.;

        egui::Frame::none()
            .inner_margin(egui::style::Margin {
                left: inner_margin_x,
                right: inner_margin_x,
                top: inner_margin_y,
                bottom: inner_margin_y,
            })
            // .fill(egui::Color32::from_rgb(119, 119, 119))
            .show(ui, |ui| ui.add(egui::Spinner::new().size(spinner_size)));
    }
}

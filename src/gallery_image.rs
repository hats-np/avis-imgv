use crate::image::Image;
use crate::metadata;
use eframe::egui::{self, vec2, Rect, RichText};
use eframe::epaint::{Pos2, Vec2};
use std;
use std::path::PathBuf;
use std::thread::JoinHandle;

pub struct GalleryImageSizing {
    pub zoom_factor: f32,
    pub scroll_delta: Vec2,
    pub should_maximize: bool,
    pub has_maximized: bool,
}

pub struct GalleryImageFrame {
    pub enabled: bool,
    pub size_r: f32,
}

pub struct GalleryImage {
    pub path: PathBuf,
    pub name: String,
    pub display_name: Option<String>,
    scroll_pos: Pos2,
    image: Option<Image>,
    load_image_handle: Option<JoinHandle<Option<Image>>>,
    output_profile: String,
    display_metadata: Option<Vec<(String, String)>>,
    pub prev_percentage_zoom: f32,
    pub prev_available_size: Vec2,
    ///prev target size before zoom
    pub prev_target_size: Vec2,
}

impl GalleryImage {
    pub fn from_paths(
        paths: &[PathBuf],
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
                output_profile: output_profile.to_owned(),
                display_metadata: None,
                display_name: None,
                prev_percentage_zoom: 0.,
                prev_available_size: vec2(0., 0.),
                prev_target_size: vec2(0., 0.),
            })
            .collect()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        frame: &GalleryImageFrame,
        sizing: &mut GalleryImageSizing,
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

        let original_size = texture.size_vec2();
        let mut target_size = texture.size_vec2();
        let aspect_ratio = texture.aspect_ratio();

        //Fits image to available screen space, therefore images will never be cropped
        //By default
        if ui.available_width() < target_size[0] {
            target_size[0] = ui.available_width();
            target_size[1] = target_size[0] / aspect_ratio;
        }

        if ui.available_height() < target_size[1] {
            target_size[1] = ui.available_height();
            target_size[0] = aspect_ratio * target_size[1];
        }

        self.prev_target_size = target_size;
        self.prev_available_size = ui.available_size();

        //Not a fan of having this logic here but it seems like the only way to avoid having one
        //frame where the image is in its default size which causes a slight and unpleasant
        //"zoom in" effect
        if sizing.should_maximize && !sizing.has_maximized {
            sizing.has_maximized = true;
            if self.prev_available_size.x / self.prev_available_size.y
                > self.prev_target_size.x / self.prev_target_size.y
            {
                sizing.zoom_factor = self.prev_available_size.y / self.prev_target_size.y;
            } else {
                sizing.zoom_factor = self.prev_available_size.x / self.prev_target_size.x;
            }
        }

        //Scales image based on zoom
        target_size[0] *= sizing.zoom_factor;
        target_size[1] *= sizing.zoom_factor;

        //Sets zoom percentage
        self.prev_percentage_zoom = target_size[0] * 100. / original_size[0];

        let mut display_size = target_size;

        if display_size[0] > ui.available_width() {
            display_size[0] = ui.available_width();
        }

        if display_size[1] > ui.available_height() {
            display_size[1] = ui.available_height();
        }

        //Visible rect of the image (target_size)
        let mut visible_rect = Rect {
            min: Pos2 { x: 0.0, y: 0.0 },
            max: Pos2 { x: 1.0, y: 1.0 },
        };

        //Conform visible rect to display_size by cropping the image
        let out_bounds_y = target_size[1] - display_size[1];
        if out_bounds_y > 0.0 {
            let rect_offset_y = (1.0 - (display_size[1] / target_size[1])) / 2.0;
            visible_rect.min.y = rect_offset_y;
            visible_rect.max.y = 1.0 - rect_offset_y;
        }

        let out_bounds_x = target_size[0] - display_size[0];
        if out_bounds_x > 0.0 {
            let rect_offset_x = (1.0 - (display_size[0] / target_size[0])) / 2.0;
            visible_rect.min.x = rect_offset_x;
            visible_rect.max.x = 1.0 - rect_offset_x;
        }

        Self::update_scroll_pos(
            &mut self.scroll_pos,
            &visible_rect,
            &mut sizing.scroll_delta,
        );

        //Move the visible rect based on the scroll position
        visible_rect.min.y += self.scroll_pos.y / 2.0;
        visible_rect.max.y += self.scroll_pos.y / 2.0;
        visible_rect.min.x += self.scroll_pos.x / 2.0;
        visible_rect.max.x += self.scroll_pos.x / 2.0;

        if frame.enabled {
            //we use the shortest side
            let stroke = if display_size[0] > display_size[1] {
                display_size[1] * frame.size_r
            } else {
                display_size[0] * frame.size_r
            };

            let aspect_ratio = display_size[0] / display_size[1];

            //Need to do some more debugging here, behaves differently when
            //No title bar is present
            display_size[0] -= stroke;
            display_size[1] -= stroke / aspect_ratio;

            let image = egui::Image::new(texture)
                .fit_to_exact_size(vec2(display_size[0], display_size[1]))
                .maintain_aspect_ratio(false)
                .uv(visible_rect);

            let offset_x = (ui.available_width() - (display_size[0] + stroke)) / 2.;
            let offset_y = (ui.available_height() - (display_size[1] + stroke)) / 2.;
            ui.painter().rect_filled(
                Rect {
                    min: Pos2::new(offset_x, offset_y),
                    max: Pos2::new(
                        offset_x + display_size[0] + stroke,
                        offset_y + display_size[1] + stroke,
                    ),
                },
                1.,
                egui::Color32::WHITE,
            );

            ui.add(image);
        } else {
            ui.add(
                egui::Image::new(texture)
                    .uv(visible_rect)
                    .fit_to_exact_size(vec2(display_size[0], display_size[1]))
                    .maintain_aspect_ratio(false),
            );
        }
    }

    ///If there is free space, the scroll position will be moved
    fn update_scroll_pos(scroll_pos: &mut Pos2, visible_rect: &Rect, scroll_delta: &mut Vec2) {
        let free_space = Pos2::new(
            1.0 - (visible_rect.max.x - visible_rect.min.x),
            1.0 - (visible_rect.max.y - visible_rect.min.y),
        );

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
        if lih.is_finished() {
            match lih.join() {
                Ok(image) => self.image = image,
                Err(_) => println!("Failure joining load image thread"),
            }
        } else {
            self.load_image_handle = Some(lih);
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

    pub fn image_size(&self) -> Option<Vec2> {
        if let Some(img) = &self.image {
            if let Some(texture) = &img.texture {
                return Some(texture.size_vec2());
            }
        }

        None
    }

    pub fn unload(&mut self) {
        if self.image.is_some() || self.load_image_handle.is_some() {
            println!("{} -> Unloading image", self.name);
        }

        self.image = None;
        self.load_image_handle = None;
    }

    pub fn load(&mut self) {
        if self.load_image_handle.is_none() && self.image.is_none() {
            println!("{} -> Loading image", self.name);
            self.load_image_handle = Some(Image::load(
                self.path.clone(),
                None,
                self.output_profile.clone(),
            ));
        }
    }

    pub fn display_loading_frame(ui: &mut egui::Ui) {
        let spinner_size = ui.available_height() / 3.;
        let inner_margin_y = (ui.available_height() - spinner_size) / 2.;
        let inner_margin_x = (ui.available_width() - spinner_size) / 2.;

        egui::Frame::none()
            .inner_margin(egui::Margin {
                left: inner_margin_x,
                right: inner_margin_x,
                top: inner_margin_y,
                bottom: inner_margin_y,
            })
            .show(ui, |ui| ui.add(egui::Spinner::new().size(spinner_size)));
    }

    pub fn set_display_name(&mut self, format: &str) -> String {
        if format.is_empty() {
            self.display_name = Some(self.name.clone());

            return self.name.clone();
        }

        if let Some(img) = &self.image {
            let display_name =
                metadata::Metadata::format_string_with_metadata(format, &img.metadata);

            self.display_name = Some(display_name.clone());

            display_name
        } else {
            String::new()
        }
    }

    pub fn get_display_name(&mut self, format: String) -> String {
        match &self.display_name {
            Some(dn) => dn.clone(),
            None => self.set_display_name(&format),
        }
    }

    pub fn is_loading(&self) -> bool {
        self.load_image_handle.is_some()
    }
}

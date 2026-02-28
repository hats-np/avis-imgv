use crate::image_store::ImageStore;
use crate::metadata;
use eframe::egui::load::SizedTexture;
use eframe::egui::{self, Rect, RichText, Widget, vec2};
use eframe::epaint::{Pos2, Vec2};
use std;
use std::path::PathBuf;

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
    display_metadata: Option<Vec<(String, String)>>,
    pub prev_percentage_zoom: f32,
    pub prev_available_size: Vec2,
    ///prev target size before zoom
    pub prev_target_size: Vec2,
    pub prev_cursor_pos_normalized: Vec2,
    is_loaded: bool,
}

impl GalleryImage {
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
                scroll_pos: Pos2::new(0.0, 0.0),
                display_metadata: None,
                display_name: None,
                prev_percentage_zoom: 0.,
                prev_available_size: vec2(0., 0.),
                prev_target_size: vec2(0., 0.),
                prev_cursor_pos_normalized: vec2(0., 0.),
                is_loaded: false,
            })
            .collect()
    }

    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        frame: &GalleryImageFrame,
        sizing: &mut GalleryImageSizing,
        image_store: &ImageStore,
    ) {
        let image_size = match image_store.get_image_size(&self.path) {
            Some(is) => is,
            None => {
                Self::display_loading_frame(ui);
                return;
            }
        };

        let texture_id = match image_store.get_texture_id(&self.path) {
            Some(is) => is,
            None => {
                Self::display_loading_frame(ui);
                return;
            }
        };

        self.is_loaded = true;

        let original_size = image_size;
        let mut target_size = image_size;
        let aspect_ratio = image_size.x / image_size.y;

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
            max: Pos2 {
                x: display_size.x,
                y: display_size.y,
            },
        };

        //Conform visible rect to display_size by cropping the image
        let out_bounds_y = target_size[1] - display_size[1];
        let out_bounds_x = target_size[0] - display_size[0];

        if out_bounds_y > 0.0 {
            let remain_y = (target_size.y - display_size.y) / 2.0;
            visible_rect.min.y = remain_y;
            visible_rect.max.y = target_size.y - remain_y;
        }

        if out_bounds_x > 0.0 {
            let remain_x = (target_size.x - display_size.x) / 2.0;
            visible_rect.min.x = remain_x;
            visible_rect.max.x = target_size.x - remain_x;
        }

        Self::update_panning_pos(
            &mut self.scroll_pos,
            &mut visible_rect,
            &target_size,
            &mut sizing.scroll_delta,
        );

        let visible_rect_normalized = Rect {
            min: Pos2 {
                x: visible_rect.min.x / target_size.x,
                y: visible_rect.min.y / target_size.y,
            },
            max: Pos2 {
                x: visible_rect.max.x / target_size.x,
                y: visible_rect.max.y / target_size.y,
            },
        };

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

            let image = egui::Image::new(SizedTexture::new(texture_id, image_size))
                .fit_to_exact_size(vec2(display_size[0], display_size[1]))
                .maintain_aspect_ratio(false)
                .uv(visible_rect_normalized);

            let available = ui.available_rect_before_wrap();
            let offset_x = available.center().x - (display_size[0] + stroke) / 2.0;
            let offset_y = available.center().y - (display_size[1] + stroke) / 2.0;

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
            egui::Image::new(SizedTexture::new(texture_id, image_size))
                .uv(visible_rect_normalized)
                .fit_to_exact_size(vec2(display_size[0], display_size[1]))
                .maintain_aspect_ratio(false)
                .ui(ui);
        }
    }

    ///If there is free space, the scroll position will be moved
    fn update_panning_pos(
        scroll_pos: &mut Pos2,
        visible_rect: &mut Rect,
        target_size: &Vec2,
        scroll_delta: &mut Vec2,
    ) {
        let free_space = Pos2::new(
            target_size.x - (visible_rect.max.x - visible_rect.min.x),
            target_size.y - (visible_rect.max.y - visible_rect.min.y),
        );

        //reverse scroll directions
        scroll_delta.x *= -1.0;
        scroll_delta.y *= -1.0;

        if free_space.x != 0.0 {
            //has available space to scroll in the x direction
            scroll_pos.x += scroll_delta.x;
            if scroll_pos.x > free_space.x / 2.0 {
                scroll_pos.x = free_space.x / 2.0;
            } else if scroll_pos.x < -free_space.x / 2.0 {
                scroll_pos.x = -free_space.x / 2.0;
            }
        } else {
            scroll_pos.x = 0.0;
        }

        if free_space.y != 0.0 {
            //has available space to scroll in the y direction
            scroll_pos.y += scroll_delta.y;
            if scroll_pos.y > free_space.y / 2.0 {
                scroll_pos.y = free_space.y / 2.0;
            } else if scroll_pos.y < -free_space.y / 2.0 {
                scroll_pos.y = -free_space.y / 2.0;
            }
        } else {
            scroll_pos.y = 0.0;
        }

        visible_rect.min.y += scroll_pos.y;
        visible_rect.max.y += scroll_pos.y;
        visible_rect.min.x += scroll_pos.x;
        visible_rect.max.x += scroll_pos.x;
    }

    pub fn metadata_ui(
        &mut self,
        ui: &mut egui::Ui,
        tags_to_display: &Vec<String>,
        image_store: &ImageStore,
    ) {
        if let Some(metadata) = image_store.get_image_metadata(&self.path) {
            if self.display_metadata.is_none() {
                let mut display_metadata: Vec<(String, String)> = vec![];
                for tag in tags_to_display {
                    if let Some(value) = metadata.get(tag) {
                        display_metadata.push((tag.to_string(), value.to_string()));
                    };
                }
                self.display_metadata = Some(display_metadata);
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

    pub fn image_size(&self, image_store: ImageStore) -> Option<Vec2> {
        image_store.get_image_size(&self.path)
    }

    pub fn display_loading_frame(ui: &mut egui::Ui) {
        let spinner_size = ui.available_height() / 3.;
        let inner_margin_y = (ui.available_height() - spinner_size) / 2.;
        let inner_margin_x = (ui.available_width() - spinner_size) / 2.;

        egui::Frame::NONE
            .inner_margin(epaint::MarginF32 {
                left: inner_margin_x,
                right: inner_margin_x,
                top: inner_margin_y,
                bottom: inner_margin_y,
            })
            .show(ui, |ui| ui.add(egui::Spinner::new().size(spinner_size)));
    }

    pub fn set_display_name(&mut self, format: &str, image_store: &ImageStore) -> String {
        if format.is_empty() {
            self.display_name = Some(self.name.clone());

            return self.name.clone();
        }

        if let Some(metadata) = image_store.get_image_metadata(&self.path) {
            let display_name = metadata::Metadata::format_string_with_metadata(format, metadata);

            self.display_name = Some(display_name.clone());

            display_name
        } else {
            String::new()
        }
    }

    pub fn get_display_name(&mut self, format: String, image_store: &ImageStore) -> String {
        match &self.display_name {
            Some(dn) => dn.clone(),
            None => self.set_display_name(&format, image_store),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.is_loaded
    }
}

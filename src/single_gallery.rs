use eframe::egui::Sense;
use eframe::{egui, epaint::Vec2};
use std::path::{Path, PathBuf};

use crate::gallery_image::{GalleryImageFrame, GalleryImageSizing};
use crate::{
    callback::Callback,
    config::GalleryConfig,
    gallery_image::GalleryImage,
    no_icon,
    user_action::{self, show_context_menu},
    utils, Order,
};

pub const PERCENTAGES: &[f32] = &[200., 100., 75., 50., 25.];

pub struct SingleGallery {
    imgs: Vec<GalleryImage>,
    pub selected_img_index: usize,
    metadata_pannel_visible: bool,
    preload_active: bool,
    frame: GalleryImageFrame,
    sizing: GalleryImageSizing,
    config: GalleryConfig,
    jump_to: String,
    output_profile: String,
    callback: Option<Callback>,
}

impl SingleGallery {
    pub fn new(
        image_paths: &[PathBuf],
        selected_image_path: &Option<PathBuf>,
        config: GalleryConfig,
        output_profile: &String,
    ) -> SingleGallery {
        let mut sg = SingleGallery {
            imgs: vec![],
            selected_img_index: 0,
            preload_active: true,
            frame: GalleryImageFrame {
                enabled: false,
                size_r: config.frame_size_relative_to_image,
            },
            sizing: GalleryImageSizing {
                zoom_factor: 1.0,
                scroll_delta: Vec2::new(0., 0.),
                should_maximize: false,
                has_maximized: false,
            },
            metadata_pannel_visible: false,
            jump_to: String::new(),
            output_profile: output_profile.to_owned(),
            callback: None,
            config,
        };

        sg.set_images(image_paths, selected_image_path);

        sg
    }

    pub fn set_images(&mut self, image_paths: &[PathBuf], selected_image_path: &Option<PathBuf>) {
        let imgs = GalleryImage::from_paths(image_paths, &self.output_profile);

        self.imgs = imgs;
        self.selected_img_index = match selected_image_path {
            Some(path) => self.imgs.iter().position(|x| &x.path == path).unwrap_or(0),
            None => 0,
        };
        self.preload_active =
            Self::is_valid_for_preload(self.config.nr_loaded_images, self.imgs.len());

        println!(
            "Starting gallery with {} images on image {}",
            self.imgs.len(),
            self.selected_img_index + 1
        );

        self.load();
    }

    pub fn load(&mut self) {
        if self.imgs.is_empty() {
            return;
        }

        if !self.preload_active {
            for i in 0..self.imgs.len() {
                self.imgs[i].load();
            }

            return;
        }

        //Not many entries in this vec so it's not worth to use a hasmap
        let mut indexes_to_load: Vec<usize> = vec![self.selected_img_index];

        for i in 1..self.config.nr_loaded_images + 1 {
            indexes_to_load.push(get_vec_index_subtracted_by(
                self.imgs.len(),
                self.selected_img_index,
                i,
            ));
            indexes_to_load.push(get_vec_index_sum_by(
                self.imgs.len(),
                self.selected_img_index,
                i,
            ));
        }

        for (i, img) in &mut self.imgs.iter_mut().enumerate() {
            if indexes_to_load.contains(&i) {
                img.load();
            } else {
                img.unload();
            }
        }
    }

    pub fn select_by_name(&mut self, img_name: String) {
        self.selected_img_index = self
            .imgs
            .iter()
            .position(|x| x.name == img_name)
            .unwrap_or(0);

        self.load();
    }

    pub fn next_image(&mut self) {
        if self.imgs.is_empty() {
            return;
        }

        if self.config.should_wait && self.active_img_is_loading() {
            return;
        }

        if self.preload_active {
            let index_to_clear = get_vec_index_subtracted_by(
                self.imgs.len(),
                self.selected_img_index,
                self.config.nr_loaded_images,
            );

            let index_to_preload = get_vec_index_sum_by(
                self.imgs.len(),
                self.selected_img_index,
                self.config.nr_loaded_images,
            );

            self.imgs[index_to_clear].unload();
            self.imgs[index_to_preload].load();
        }

        if self.selected_img_index == self.imgs.len() - 1 {
            self.selected_img_index = 0;
        } else {
            self.selected_img_index += 1;
        }

        self.sizing.has_maximized = false;
    }

    pub fn previous_image(&mut self) {
        if self.imgs.is_empty() {
            return;
        }

        if self.preload_active {
            let index_to_clear = get_vec_index_sum_by(
                self.imgs.len(),
                self.selected_img_index,
                self.config.nr_loaded_images,
            );

            let index_to_preload = get_vec_index_subtracted_by(
                self.imgs.len(),
                self.selected_img_index,
                self.config.nr_loaded_images,
            );

            self.imgs[index_to_clear].unload();
            self.imgs[index_to_preload].load();
        }

        if self.selected_img_index == 0 {
            self.selected_img_index = self.imgs.len() - 1;
        } else {
            self.selected_img_index -= 1;
        }

        self.sizing.has_maximized = false;
    }

    pub fn toggle_frame(&mut self) {
        self.frame.enabled = !self.frame.enabled;
    }

    pub fn toggle_metadata(&mut self) {
        self.metadata_pannel_visible = !self.metadata_pannel_visible;
    }

    pub fn reset_zoom(&mut self) {
        self.sizing.zoom_factor = 1.0;
    }

    pub fn double_zoom(&mut self) {
        if self.sizing.zoom_factor <= 7.0 {
            //make limit a config
            self.sizing.zoom_factor *= 2.0;
        } else {
            self.sizing.zoom_factor = 1.0;
        }
    }

    pub fn multiply_zoom(&mut self, zoom_delta: f32) {
        if zoom_delta != 1.0 {
            self.sizing.zoom_factor *= zoom_delta;
        }
    }

    ///Sets zoom factor based on percentage and opened image size
    pub fn set_zoom_factor_from_percentage(&mut self, percentage: &f32) {
        let img = match self.get_active_img() {
            Some(img) => img,
            None => return,
        };

        let original_size = match img.image_size() {
            Some(org_size) => org_size,
            None => return,
        };

        self.sizing.zoom_factor = ((original_size[0] * percentage / 100.)
            * self.sizing.zoom_factor)
            / (img.prev_target_size[0] * self.sizing.zoom_factor);
    }

    pub fn fit_vertical(&mut self) {
        let img = match self.get_active_img() {
            Some(img) => img,
            None => return,
        };

        self.sizing.zoom_factor = img.prev_available_size.y / img.prev_target_size.y;
    }

    pub fn fit_horizontal(&mut self) {
        let img = match self.get_active_img() {
            Some(img) => img,
            None => return,
        };

        self.sizing.zoom_factor = img.prev_available_size.x / img.prev_target_size.x;
    }

    pub fn fit_maximize(&mut self) {
        let img = match self.get_active_img() {
            Some(img) => img,
            None => return,
        };

        if img.prev_available_size.x / img.prev_available_size.y
            > img.prev_target_size.x / img.prev_target_size.y
        {
            self.sizing.zoom_factor = img.prev_available_size.y / img.prev_target_size.y;
        } else {
            self.sizing.zoom_factor = img.prev_available_size.x / img.prev_target_size.x;
        }
    }

    pub fn latch_fit_maximize(&mut self) {
        self.sizing.should_maximize = !self.sizing.should_maximize;
    }

    pub fn get_active_img_nr(&mut self) -> usize {
        self.selected_img_index + 1
    }

    pub fn get_active_img_mut(&mut self) -> Option<&mut GalleryImage> {
        if !self.imgs.is_empty() {
            return Some(&mut self.imgs[self.selected_img_index]);
        }

        None
    }

    pub fn get_active_img(&self) -> Option<&GalleryImage> {
        if !self.imgs.is_empty() {
            return Some(&self.imgs[self.selected_img_index]);
        }

        None
    }

    pub fn get_active_img_name(&mut self) -> String {
        let format = self.config.name_format.clone();
        match self.get_active_img_mut() {
            Some(img) => img.get_display_name(format),
            None => "".to_string(),
        }
    }

    pub fn get_active_img_path(&self) -> Option<PathBuf> {
        self.get_active_img().map(|img| img.path.clone())
    }

    pub fn active_img_is_loading(&self) -> bool {
        match self.get_active_img() {
            Some(img) => img.is_loading(),
            None => false,
        }
    }

    pub fn jump_to_image(&mut self) {
        self.selected_img_index = match self.jump_to.parse::<usize>() {
            Ok(i) => {
                if i > self.imgs.len() || i < 1 {
                    self.selected_img_index
                } else {
                    i - 1
                }
            }
            Err(_) => self.selected_img_index,
        };

        self.load();
        self.jump_to.clear();
    }

    pub fn reload_at(&mut self, path: &Path) {
        if let Some(index) = self.imgs.iter().position(|x| x.path == path) {
            let img = &mut self.imgs[index];
            img.unload();
            img.load();
        }
    }

    ///Pops image from the collection
    pub fn pop(&mut self, path: &Path) {
        if let Some(pos) = self.imgs.iter().position(|x| x.path == path) {
            self.imgs.remove(pos);
            self.preload_active =
                Self::is_valid_for_preload(self.config.nr_loaded_images, self.imgs.len());

            //Last image of the collection, we want to load backwards
            if self.selected_img_index == self.imgs.len() {
                self.selected_img_index = self.imgs.len() - 1;
            }

            self.load();
        }
    }

    pub fn is_valid_for_preload(preload_nr: usize, image_count: usize) -> bool {
        preload_nr * 2 <= image_count
    }

    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        order: &mut Order,
        order_changed: &mut bool,
        flattened: bool,
        watcher_enabled: bool,
    ) {
        self.handle_input(ctx);

        egui::TopBottomPanel::bottom("gallery_bottom")
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let response = ui.add_sized(
                        Vec2::new(65., ui.available_height()),
                        egui::TextEdit::singleline(&mut self.jump_to),
                    );

                    if response.lost_focus()
                        && response.ctx.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        self.jump_to_image();
                    }

                    ui.add_sized(
                        Vec2::new(35., ui.available_height()),
                        egui::Label::new(format!(
                            "{}/{}",
                            self.get_active_img_nr(),
                            self.imgs.len()
                        )),
                    );

                    egui::ComboBox::from_id_source("order_combo_box")
                        .width(110.)
                        .icon(no_icon)
                        .selected_text(order.to_string())
                        .show_ui(ui, |ui| {
                            let orders = [
                                Order::Asc,
                                Order::Desc,
                                Order::DateAsc,
                                Order::DateDesc,
                                Order::Random,
                            ];

                            for o in orders {
                                if ui
                                    .selectable_value(order, o.clone(), o.to_string())
                                    .clicked()
                                {
                                    *order_changed = true;
                                }
                            }
                        });

                    if flattened {
                        ui.label("Flattened");
                    }

                    if watcher_enabled {
                        ui.label("Watching");
                    }

                    if self.sizing.should_maximize {
                        ui.label("Maximizing");
                    }

                    let mut label = egui::Label::new(self.get_active_img_name());
                    label = label.truncate();
                    ui.add_sized(
                        Vec2::new(ui.available_width() - 245., ui.available_height()),
                        label,
                    );

                    ui.with_layout(
                        egui::Layout::right_to_left(eframe::emath::Align::Max),
                        |ui| {
                            ui.add_sized(
                                Vec2::new(200., ui.available_height()),
                                egui::Slider::new(&mut self.sizing.zoom_factor, 0.5..=10.0)
                                    .text("🔎"),
                            );

                            if let Some(img) = self.get_active_img() {
                                let resp = ui.add_sized(
                                    Vec2::new(45., ui.available_height()),
                                    egui::Label::new(format!("{:.1}%", img.prev_percentage_zoom))
                                        .sense(Sense::click()),
                                );

                                resp.context_menu(|ui| {
                                    if ui.button("Fit to screen").clicked() {
                                        self.reset_zoom();
                                        ui.close_menu();
                                    }

                                    if ui.button("Fit horizontal").clicked() {
                                        self.fit_horizontal();
                                        ui.close_menu();
                                    }

                                    if ui.button("Fit vertical").clicked() {
                                        self.fit_vertical();
                                        ui.close_menu();
                                    }

                                    ui.separator();

                                    for percentage in PERCENTAGES {
                                        if ui.button(format!("{:.0}%", percentage)).clicked() {
                                            self.set_zoom_factor_from_percentage(percentage);
                                            ui.close_menu();
                                        }
                                    }
                                });
                            }
                        },
                    )
                });
            });

        egui::SidePanel::left("image_metadata")
            .resizable(true)
            .default_width(500.)
            .show_separator_line(false)
            .show_animated(ctx, self.metadata_pannel_visible, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.heading("Image Metadata");
                    ui.add(egui::Separator::default());
                    self.imgs[self.selected_img_index].metadata_ui(ui, &self.config.metadata_tags);
                })
            });

        let image_pannel_resp = egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(119, 119, 119)))
            .show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    if !self.imgs.is_empty() {
                        let img: &mut GalleryImage = &mut self.imgs[self.selected_img_index];
                        img.ui(ui, &self.frame, &mut self.sizing);
                    }
                });
            })
            .response;

        //unfortunately we'll always be one frame behind
        //when advancing with the scroll wheel
        if image_pannel_resp.contains_pointer() {
            if self.config.scroll_navigation {
                if ctx.input(|i| i.raw_scroll_delta.y) > 0.0 && ctx.input(|i| i.zoom_delta()) == 1.0
                {
                    self.next_image();
                }

                if ctx.input(|i| i.raw_scroll_delta.y) < 0.0 && ctx.input(|i| i.zoom_delta()) == 1.0
                {
                    self.previous_image();
                }
            }

            self.sizing.scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
            if ctx.input(|i| i.pointer.any_down()) {
                //drag
                let pointer_delta = ctx.input(|i| i.pointer.delta());
                self.sizing.scroll_delta += pointer_delta * 0.5;
            }
        } else {
            //lest we lose hover in the frame that there's a scroll
            //delta and we get infinite zoom
            self.sizing.scroll_delta.x = 0.;
            self.sizing.scroll_delta.y = 0.;
        }

        if let Some(path) = self.get_active_img_path() {
            let callback = show_context_menu(&self.config.context_menu, image_pannel_resp, &path);

            if let Some(callback) = callback {
                self.callback = Some(Callback::from_callback(callback, Some(path)));
            }
        }
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) {
        if utils::are_inputs_muted(ctx) {
            return;
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_fit.kbd_shortcut)) {
            self.reset_zoom();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_frame.kbd_shortcut)) {
            self.toggle_frame();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_zoom.kbd_shortcut)) {
            self.double_zoom();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_metadata.kbd_shortcut)) {
            self.toggle_metadata();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_next.kbd_shortcut)) {
            self.next_image();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_prev.kbd_shortcut)) {
            self.previous_image();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_one_to_one.kbd_shortcut)) {
            self.set_zoom_factor_from_percentage(&100.);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_fit_horizontal.kbd_shortcut)) {
            self.fit_horizontal();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_fit_vertical.kbd_shortcut)) {
            self.fit_vertical();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_fit_maximize.kbd_shortcut)) {
            self.fit_maximize();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_latch_fit_maximize.kbd_shortcut)) {
            self.latch_fit_maximize();
        }

        self.multiply_zoom(ctx.input(|i| i.zoom_delta()));

        for action in &self.config.user_actions {
            if !ctx.input_mut(|i| i.consume_shortcut(&action.shortcut.kbd_shortcut)) {
                continue;
            }

            if let Some(path) = self.get_active_img_path() {
                if user_action::execute(&action.exec, &path) {
                    if let Some(callback) = action.callback.to_owned() {
                        self.callback =
                            Some(Callback::from_callback(callback, Some(path.to_owned())));
                    }
                }
            } else {
                println!("Unable to get active image path for user action");
            }
        }
    }

    pub fn take_callback(&mut self) -> Option<Callback> {
        self.callback.take()
    }
}

fn get_vec_index_subtracted_by(vec_len: usize, current_index: usize, to_subtract: usize) -> usize {
    if current_index < to_subtract {
        vec_len - (to_subtract - current_index)
    } else {
        current_index - to_subtract
    }
}

fn get_vec_index_sum_by(vec_len: usize, current_index: usize, to_sum: usize) -> usize {
    let mut idx = current_index + to_sum;
    if idx >= vec_len {
        idx = to_sum - (vec_len - current_index);
    }

    idx
}

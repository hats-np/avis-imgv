use eframe::egui::{Response, Sense};
use eframe::{egui, epaint::Vec2};
use std::cmp::min;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config::SlideshowConfig;
use crate::gallery_image::{GalleryImageFrame, GalleryImageSizing};
use crate::image_store::ImageStore;
use crate::{
    callback::Callback,
    config::ImageViewConfig,
    gallery_image::GalleryImage,
    user_action::{self, show_context_menu},
    utils,
};

pub const PERCENTAGES: &[f32] = &[200., 100., 75., 50., 25.];

#[derive(Clone)]
pub struct Slideshow {
    last_adv_instant: Instant,
    last_zoom_instant: Instant,
    zoom_step: Option<f32>,
    zoom_step_ms: u128,
    zoom_step_count: f32,
}

impl Slideshow {
    pub fn new(cfg: &SlideshowConfig) -> Slideshow {
        //For now we use a hardcoded step count. Zoom every 50ms to conserve energy
        let total_zoom_steps = cfg.seconds_per_image as f32 * 20.;

        Slideshow {
            last_adv_instant: Instant::now(),
            last_zoom_instant: Instant::now(),
            zoom_step_ms: 50,
            zoom_step: None,
            zoom_step_count: total_zoom_steps,
        }
    }

    //when the slideshow advances to the next image it should always display it maximized in  the screen,
    // aka, no cropping. This can lead to different %zoom baselines for each image
    pub fn set_zoom_step(&mut self, percent_zoom: f32, active_image: Option<&GalleryImage>) {
        if percent_zoom != 0.
            && let Some(active_image) = active_image
        {
            //in the very first frame of the app this value is always 0.0
            if active_image.prev_percentage_zoom != 0.0 {
                self.zoom_step = Some(
                    (active_image.prev_percentage_zoom * percent_zoom / self.zoom_step_count)
                        / 100.,
                );
            }
        }
    }
}

pub struct ImageView {
    imgs: Vec<GalleryImage>,
    pub selected_img_index: usize,
    preload_active: bool,
    frame: GalleryImageFrame,
    sizing: GalleryImageSizing,
    config: ImageViewConfig,
    jump_to: String,
    callback: Option<Callback>,
    nr_images_displayed: usize,
    slideshow_config: SlideshowConfig,
    slideshow: Option<Slideshow>,
}

impl ImageView {
    pub fn new(
        image_paths: &[PathBuf],
        selected_image_path: &Option<PathBuf>,
        config: ImageViewConfig,
        start_slideshow: bool,
        slideshow_config: SlideshowConfig,
        image_store: &mut ImageStore,
    ) -> ImageView {
        let mut gallery_sizing = GalleryImageSizing {
            zoom_factor: 1.0,
            scroll_delta: Vec2::new(0., 0.),
            should_maximize: false,
            has_maximized: false,
        };

        let mut frame = GalleryImageFrame {
            enabled: false,
            size_r: config.frame_size_relative_to_image,
        };

        let slideshow: Option<Slideshow>;

        if start_slideshow {
            slideshow = Some(Slideshow::new(&slideshow_config));
            gallery_sizing.should_maximize = true;

            if slideshow_config.start_with_frame_enabled {
                frame.enabled = true;
            }
        } else {
            slideshow = None;
        };

        let mut sg = ImageView {
            imgs: vec![],
            selected_img_index: 0,
            preload_active: true,
            frame,
            sizing: gallery_sizing,
            jump_to: String::new(),
            callback: None,
            nr_images_displayed: config.nr_images_shown,
            config: config.clone(),
            slideshow_config,
            slideshow,
        };

        sg.set_images(image_paths, selected_image_path, image_store);

        sg
    }

    pub fn set_images(
        &mut self,
        image_paths: &[PathBuf],
        selected_image_path: &Option<PathBuf>,
        image_store: &mut ImageStore,
    ) {
        for img in &self.imgs {
            image_store.deregister_img(&img.path);
        }

        let imgs = GalleryImage::from_paths(image_paths);

        self.imgs = imgs;
        self.selected_img_index = match selected_image_path {
            Some(path) => self.imgs.iter().position(|x| &x.path == path).unwrap_or(0),
            None => 0,
        };
        self.preload_active =
            Self::is_valid_for_preload(self.config.nr_loaded_images, self.imgs.len());

        tracing::info!(
            "Starting gallery with {} images on image {}",
            self.imgs.len(),
            self.selected_img_index + 1
        );

        self.load(image_store);
    }

    pub fn load(&mut self, image_store: &mut ImageStore) {
        if self.imgs.is_empty() {
            return;
        }

        if !self.preload_active {
            for i in 0..self.imgs.len() {
                image_store.register_img(&self.imgs[i].path, None);
            }

            return;
        }

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
                image_store.register_img(&img.path, None);
            } else {
                image_store.deregister_img(&img.path)
            }
        }
    }

    pub fn select_by_name(&mut self, img_name: String, image_store: &mut ImageStore) {
        self.selected_img_index = self
            .imgs
            .iter()
            .position(|x| x.name == img_name)
            .unwrap_or(0);

        self.load(image_store);
    }

    pub fn next_image(&mut self, image_store: &mut ImageStore) {
        if self.imgs.is_empty() {
            return;
        }

        if self.config.should_wait && self.active_img_is_loading(image_store) {
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

            image_store.deregister_img(&self.imgs[index_to_clear].path);
            image_store.register_img(&self.imgs[index_to_preload].path, None);
        }

        if self.selected_img_index == self.imgs.len() - 1 {
            self.selected_img_index = 0;
        } else {
            self.selected_img_index += 1;
        }

        self.sizing.has_maximized = false;
    }

    pub fn previous_image(&mut self, image_store: &mut ImageStore) {
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

            image_store.deregister_img(&self.imgs[index_to_clear].path);
            image_store.register_img(&self.imgs[index_to_preload].path, None);
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
    pub fn set_zoom_factor_from_percentage(&mut self, percentage: &f32, image_store: &ImageStore) {
        let img = match self.get_active_img() {
            Some(img) => img,
            None => return,
        };

        let original_size = match image_store.get_image_size(&img.path) {
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

    pub fn get_active_img_name(&mut self, image_store: &ImageStore) -> String {
        let format = self.config.name_format.clone();
        match self.get_active_img_mut() {
            Some(img) => img.get_display_name(format, image_store),
            None => "".to_string(),
        }
    }

    pub fn get_active_img_path(&self) -> Option<PathBuf> {
        self.get_active_img().map(|img| img.path.clone())
    }

    pub fn active_img_is_loading(&self, image_store: &ImageStore) -> bool {
        match self.get_active_img() {
            Some(img) => !image_store.is_image_loaded(&img.path),
            None => false,
        }
    }

    pub fn jump_to_image(&mut self, image_store: &mut ImageStore) {
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

        self.load(image_store);
        self.jump_to.clear();
    }

    pub fn reload_at(&mut self, path: &Path, image_store: &mut ImageStore) {
        if let Some(index) = self.imgs.iter().position(|x| x.path == path) {
            let img = &mut self.imgs[index];
            image_store.reload(&img.path, None);
        }
    }

    //TODO: Manage this outside of his view.
    ///Pops image from the collection
    pub fn pop(&mut self, path: &Path, image_store: &mut ImageStore) {
        if let Some(pos) = self.imgs.iter().position(|x| x.path == path) {
            self.imgs.remove(pos);
            self.preload_active =
                Self::is_valid_for_preload(self.config.nr_loaded_images, self.imgs.len());

            //Last image of the collection, we want to load backwards
            if self.selected_img_index == self.imgs.len() {
                self.selected_img_index = self.imgs.len() - 1;
            }

            self.load(image_store);
        }
    }

    pub fn is_valid_for_preload(preload_nr: usize, image_count: usize) -> bool {
        preload_nr * 2 <= image_count
    }

    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        flattened: bool,
        watcher_enabled: bool,
        image_store: &mut ImageStore,
    ) {
        self.handle_input(ctx, image_store);

        //In slideshow mode we only want to see the picture
        if self.slideshow.is_none() {
            self.show_view_bottom_bar(ctx, flattened, watcher_enabled, image_store);
        } else {
            self.handle_slideshow(ctx, image_store);
        }

        let show_image_response = self.show_image(ctx, image_store);
        self.handle_image_scroll(ctx, &show_image_response, image_store);
        self.handle_callbacks(&show_image_response);
    }

    pub fn handle_input(&mut self, ctx: &egui::Context, image_store: &mut ImageStore) {
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
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_next.kbd_shortcut)) {
            self.next_image(image_store);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_prev.kbd_shortcut)) {
            self.previous_image(image_store);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_one_to_one.kbd_shortcut)) {
            self.set_zoom_factor_from_percentage(&100., image_store);
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
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_more_images_shown.kbd_shortcut))
            && self.nr_images_displayed < self.config.nr_loaded_images
        {
            self.nr_images_displayed += 1;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_less_images_shown.kbd_shortcut))
            && self.nr_images_displayed > 1
        {
            self.nr_images_displayed -= 1;
        }

        self.multiply_zoom(ctx.input(|i| i.zoom_delta()));

        for action in &self.config.user_actions {
            if !ctx.input_mut(|i| i.consume_shortcut(&action.shortcut.kbd_shortcut)) {
                continue;
            }

            if let Some(path) = self.get_active_img_path() {
                if user_action::execute(&action.exec, &path)
                    && let Some(callback) = action.callback.to_owned()
                {
                    self.callback = Some(Callback::from_callback(callback, Some(path.to_owned())));
                }
            } else {
                tracing::error!("Unable to get active image path for user action");
            }
        }
    }

    pub fn take_callback(&mut self) -> Option<Callback> {
        self.callback.take()
    }

    pub fn show_image(&mut self, ctx: &egui::Context, image_store: &ImageStore) -> Response {
        egui::CentralPanel::default()
            .frame(self.get_image_frame())
            .show(ctx, |ui| {
                if !self.imgs.is_empty() {
                    if self.imgs.len() == 1 {
                        ui.centered_and_justified(|ui| {
                            let img: &mut GalleryImage = &mut self.imgs[self.selected_img_index];
                            img.ui(ui, &self.frame, &mut self.sizing, image_store);
                        });
                    } else {
                        let w = (ui.available_width() / self.nr_images_displayed as f32) - 1.;
                        let h = ui.available_height();
                        ui.horizontal(|ui| {
                            let nr_images_to_display =
                                min(self.nr_images_displayed, self.imgs.len());
                            for i in 0..nr_images_to_display {
                                ui.allocate_ui(Vec2 { x: w, y: h }, |ui| {
                                    ui.centered_and_justified(|ui| {
                                        let index = get_vec_index_sum_by(
                                            self.imgs.len(),
                                            self.selected_img_index,
                                            i,
                                        );
                                        let img: &mut GalleryImage = &mut self.imgs[index];
                                        img.ui(ui, &self.frame, &mut self.sizing, image_store);
                                    });
                                });
                            }
                        });
                    }
                }
            })
            .response
            .interact(Sense::click())
    }

    pub fn get_image_frame(&mut self) -> egui::Frame {
        let mut background_color = egui::Color32::from_rgb(119, 119, 119);

        if self.slideshow.is_some()
            && let Some(override_hex) = self
                .slideshow_config
                .image_frame_background_color_override
                .as_ref()
        {
            background_color = egui::Color32::from_hex(override_hex).unwrap_or(background_color);
        }

        egui::Frame::NONE.fill(background_color)
    }

    pub fn handle_image_scroll(
        &mut self,
        ctx: &egui::Context,
        response: &Response,
        image_store: &mut ImageStore,
    ) {
        //unfortunately we'll always be one frame behind
        //when advancing with the scroll wheel
        if response.contains_pointer() {
            if self.config.scroll_navigation {
                if ctx.input(|i| i.raw_scroll_delta.y) > 0.0 && ctx.input(|i| i.zoom_delta()) == 1.0
                {
                    self.next_image(image_store);
                }

                if ctx.input(|i| i.raw_scroll_delta.y) < 0.0 && ctx.input(|i| i.zoom_delta()) == 1.0
                {
                    self.previous_image(image_store);
                }
            }

            self.sizing.scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
            if ctx.input(|i| i.pointer.is_decidedly_dragging()) {
                //drag
                self.sizing.scroll_delta +=
                    ctx.input(|i| i.pointer.delta()) * ctx.pixels_per_point();
            }
        } else {
            //lest we lose hover in the frame that there's a scroll
            //delta and we get infinite zoom
            self.sizing.scroll_delta.x = 0.;
            self.sizing.scroll_delta.y = 0.;
        }
    }

    pub fn handle_callbacks(&mut self, response: &Response) {
        if let Some(path) = self.get_active_img_path() {
            let callback = show_context_menu(&self.config.context_menu, response, &path);

            if let Some(callback) = callback {
                self.callback = Some(Callback::from_callback(callback, Some(path)));
            }
        }
    }

    pub fn handle_slideshow(&mut self, ctx: &egui::Context, image_store: &mut ImageStore) {
        if self.slideshow.is_none() {
            return;
        }

        let mut slideshow = self.slideshow.clone().unwrap();

        if slideshow.zoom_step.is_none() {
            slideshow.set_zoom_step(self.slideshow_config.percent_zoom, self.get_active_img());
        }

        let mut new_zoom_percentage = if let Some(active_image) = self.get_active_img() {
            active_image.prev_percentage_zoom
        } else {
            100.
        };

        if slideshow.last_zoom_instant.elapsed().as_millis() > slideshow.zoom_step_ms {
            slideshow.last_zoom_instant = Instant::now();
            new_zoom_percentage += slideshow.zoom_step.unwrap_or(0.);
        }

        if slideshow.last_adv_instant.elapsed().as_secs() > self.slideshow_config.seconds_per_image
        {
            slideshow.last_zoom_instant = Instant::now();
            slideshow.last_adv_instant = Instant::now();
            slideshow.zoom_step = None;
            self.next_image(image_store);
        }

        if self.slideshow_config.percent_zoom != 0. {
            self.set_zoom_factor_from_percentage(&new_zoom_percentage, image_store);
            ctx.request_repaint_after(Duration::from_millis(slideshow.zoom_step_ms as u64));
        } else {
            ctx.request_repaint_after(Duration::from_secs(self.slideshow_config.seconds_per_image));
        }

        self.slideshow = Some(slideshow);
    }

    pub fn show_view_bottom_bar(
        &mut self,
        ctx: &egui::Context,
        flattened: bool,
        watcher_enabled: bool,
        image_store: &mut ImageStore,
    ) {
        egui::TopBottomPanel::bottom("image_view_bottom_bar")
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
                        self.jump_to_image(image_store);
                    }

                    ui.add_sized(
                        Vec2::new(35., ui.available_height()),
                        egui::Label::new(format!(
                            "{}/{}",
                            self.get_active_img_nr(),
                            self.imgs.len()
                        )),
                    );

                    if flattened {
                        ui.label("Flattened");
                    }

                    if watcher_enabled {
                        ui.label("Watching");
                    }

                    if self.sizing.should_maximize {
                        ui.label("Maximizing");
                    }

                    let mut label = egui::Label::new(self.get_active_img_name(image_store));
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
                                        ui.close();
                                    }

                                    if ui.button("Fit horizontal").clicked() {
                                        self.fit_horizontal();
                                        ui.close();
                                    }

                                    if ui.button("Fit vertical").clicked() {
                                        self.fit_vertical();
                                        ui.close();
                                    }

                                    ui.separator();

                                    for percentage in PERCENTAGES {
                                        if ui.button(format!("{percentage:.0}%")).clicked() {
                                            self.set_zoom_factor_from_percentage(
                                                percentage,
                                                image_store,
                                            );
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        },
                    )
                });
            });
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

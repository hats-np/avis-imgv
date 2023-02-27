use crate::{
    config::Config, crawler, db::Db, metadata::Metadata, multi_gallery::MultiGallery,
    single_gallery::SingleGallery,
};
use eframe::egui;
use std::time::Instant;

pub struct App {
    gallery: SingleGallery,
    //used when switching between galleries
    gallery_selected_index: Option<usize>,
    multi_gallery: MultiGallery,
    perf_metrics_visible: bool,
    multi_gallery_visible: bool,
    start_of_frame: Instant,
    longest_frametime: u128,
    longest_recent_frametime: u128,
    current_frametime: u128,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply_theme(&cc.egui_ctx);
        let cfg = Config::new();

        let mut style = (*cc.egui_ctx.style()).clone();

        for t_styles in style.text_styles.iter_mut() {
            t_styles.1.size = t_styles.1.size * cfg.text_scaling;
        }

        cc.egui_ctx.set_style(style);

        let (mut img_paths, opened_img_path) = crawler::paths_from_args();

        img_paths.sort_by(|a, b| a.cmp(&b));

        match Db::init_db() {
            Ok(_) => {
                println!("Database initiated successfully");
                match Db::trim_db(&cfg.limit_cached) {
                    Ok(_) => Metadata::cache_metadata_for_images(&img_paths),
                    Err(e) => {
                        println!("Failure trimming db {}", e);
                    }
                };
            }
            Err(e) => {
                println!("Failure initiating db -> {}", e);
            }
        };

        let app = Self {
            gallery: SingleGallery::new(
                &img_paths,
                &opened_img_path,
                cfg.gallery,
                &cfg.output_icc_profile,
            ),
            gallery_selected_index: None,
            multi_gallery: MultiGallery::new(
                &img_paths,
                cfg.multi_gallery,
                &cfg.output_icc_profile,
            ),
            perf_metrics_visible: false,
            multi_gallery_visible: false,
            start_of_frame: Instant::now(),
            longest_frametime: 0,
            longest_recent_frametime: 0,
            current_frametime: 0,
        };

        app
    }

    fn calc_frametime(&mut self) {
        let frametime = self.start_of_frame.elapsed().as_millis();

        if frametime > self.longest_frametime {
            self.longest_frametime = frametime;
        }

        if frametime > 0 {
            self.longest_recent_frametime = frametime;
        }

        self.current_frametime = frametime;
    }

    fn show_frametime(&mut self, ui: &mut egui::Ui) {
        ui.monospace(format!(
            "Current: {}ms | Recent: {}ms | Longest: {}ms",
            self.current_frametime, self.longest_recent_frametime, self.longest_frametime
        ));
    }

    //Maybe have gallery show this
    fn handle_input(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(egui::Key::Q)) {
            std::process::exit(0);
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.perf_metrics_visible = !self.perf_metrics_visible;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.multi_gallery_visible = !self.multi_gallery_visible;
            self.gallery_selected_index = Some(self.gallery.selected_img_index);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.start_of_frame = Instant::now();
        self.handle_input(ctx);

        if self.perf_metrics_visible {
            egui::TopBottomPanel::top("top_pannel").show(ctx, |ui| {
                self.show_frametime(ui);
                ctx.texture_ui(ui);
            });
        }

        if self.multi_gallery_visible {
            self.multi_gallery.ui(ctx, &mut self.gallery_selected_index);
            match self.multi_gallery.selected_image_name() {
                Some(img_name) => {
                    self.gallery.select_by_name(img_name);
                    self.multi_gallery_visible = false;
                }
                None => {}
            };
        } else {
            self.gallery.ui(ctx);
        }

        self.calc_frametime();
    }
}

use crate::{
    config::{Config, Shortcut},
    crawler,
    db::Db,
    metadata::Metadata,
    multi_gallery::MultiGallery,
    navigator,
    single_gallery::SingleGallery,
    tree, utils, VALID_EXTENSIONS,
};
use eframe::egui::{self, KeyboardShortcut};
use rfd::FileDialog;
use std::{path::PathBuf, time::Instant};

pub struct App {
    gallery: SingleGallery,
    //used when switching between galleries
    gallery_selected_index: Option<usize>,
    multi_gallery: MultiGallery,
    perf_metrics_visible: bool,
    multi_gallery_visible: bool,
    top_menu_visible: bool,
    dir_tree_visible: bool,
    navigator_visible: bool,
    navigator_search: String,
    start_of_frame: Instant,
    longest_frametime: u128,
    longest_recent_frametime: u128,
    current_frametime: u128,
    sc_toggle_gallery: Shortcut,
    sc_exit: Shortcut,
    sc_menu: Shortcut,
    sc_navigator: Shortcut,
    sc_dir_tree: Shortcut,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply_theme(&cc.egui_ctx);
        let cfg = Config::new();

        let mut style = (*cc.egui_ctx.style()).clone();

        for t_styles in style.text_styles.iter_mut() {
            t_styles.1.size *= cfg.text_scaling;
        }

        cc.egui_ctx.set_style(style);

        let (mut img_paths, opened_img_path) = crawler::paths_from_args();

        img_paths.sort();

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

        Self {
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
            top_menu_visible: false,
            dir_tree_visible: false,
            navigator_visible: false,
            navigator_search: img_paths
                .get(0)
                .unwrap_or(&PathBuf::default())
                .parent()
                .unwrap_or(&PathBuf::default())
                .to_str()
                .unwrap_or_default()
                .to_string(),
            start_of_frame: Instant::now(),
            longest_frametime: 0,
            longest_recent_frametime: 0,
            current_frametime: 0,
            sc_exit: cfg.sc_exit,
            sc_toggle_gallery: cfg.sc_toggle_gallery,
            sc_menu: cfg.sc_menu,
            sc_navigator: cfg.sc_navigator,
            sc_dir_tree: cfg.sc_dir_tree,
        }
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
        if ctx.input_mut(|i| i.consume_shortcut(&self.sc_exit.kbd_shortcut)) {
            std::process::exit(0);
        }

        if utils::are_inputs_muted(ctx) {
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F10)) {
            self.perf_metrics_visible = !self.perf_metrics_visible;
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.sc_menu.kbd_shortcut)) {
            self.top_menu_visible = !self.top_menu_visible;
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.sc_toggle_gallery.kbd_shortcut)) {
            self.multi_gallery_visible = !self.multi_gallery_visible;
            self.gallery_selected_index = Some(self.gallery.selected_img_index);
        }
    }

    //Muter inputs will block all other inputs
    //This is required so typing in text boxes and the like doesn't
    //trigger shortcuts
    fn handle_input_muters(&mut self, ctx: &egui::Context) {
        let to_check: Vec<(&mut bool, &KeyboardShortcut)> = vec![
            (&mut self.navigator_visible, &self.sc_navigator.kbd_shortcut),
            (&mut self.dir_tree_visible, &self.sc_dir_tree.kbd_shortcut),
        ];

        //Assumes all muters can and will be closed with Escape
        for (val, shortcut) in to_check {
            if ctx.input_mut(|i| i.consume_shortcut(shortcut))
                || (*val && ctx.input(|i| i.key_pressed(egui::Key::Escape)))
            {
                *val = !*val;

                if *val {
                    utils::set_mute_state(ctx, true);
                    return;
                } else {
                    utils::set_mute_state(ctx, false);
                }
            }
        }
    }

    fn folder_picker(&mut self) {
        let folder = self.get_file_dialog().pick_folder();

        if let Some(folder) = folder {
            let paths = crawler::crawl(&folder);
            self.new_images(&paths, &None)
        }
    }

    fn files_picker(&mut self) {
        let files = self
            .get_file_dialog()
            .add_filter("image", VALID_EXTENSIONS)
            .pick_files();

        if files.is_none() {
            return;
        }

        if let Some(files) = files {
            if let Some(parent) = &files[0].parent() {
                let paths = crawler::crawl(parent);
                self.new_images(&paths, &Some(files[0].clone()))
            }
        }
    }

    fn get_file_dialog(&mut self) -> FileDialog {
        let mut file_dialog = FileDialog::new();

        if let Some(path) = self.gallery.get_active_img_path() {
            if let Some(parent) = path.parent() {
                file_dialog = file_dialog.set_directory(parent);
            }
        }

        file_dialog
    }

    fn new_images(&mut self, paths: &[PathBuf], selected_img: &Option<PathBuf>) {
        self.gallery.set_images(paths, selected_img);
        self.multi_gallery.set_images(paths);
        self.navigator_search = self
            .gallery
            .get_active_img_path()
            .unwrap_or_default()
            .parent()
            .unwrap_or(&PathBuf::default())
            .to_str()
            .unwrap_or_default()
            .to_string();
        Metadata::cache_metadata_for_images(paths);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.start_of_frame = Instant::now();
        self.handle_input_muters(ctx);
        self.handle_input(ctx);

        egui::TopBottomPanel::top("top_pannel")
            .show_separator_line(false)
            .show_animated(ctx, self.perf_metrics_visible, |ui| {
                self.show_frametime(ui);
                ctx.texture_ui(ui);
            });

        egui::TopBottomPanel::top("menu")
            .show_separator_line(false)
            .show_animated(ctx, self.top_menu_visible, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Folder").clicked() {
                        self.folder_picker();
                        ui.close_menu();
                    }

                    if ui.button("Open Files").clicked() {
                        self.files_picker();
                        ui.close_menu();
                    }
                });
            });

        if self.navigator_visible && navigator::ui(&mut self.navigator_search, ctx) {
            self.navigator_visible = false;
            utils::set_mute_state(ctx, false);
            self.new_images(
                &crawler::crawl(&PathBuf::from(&self.navigator_search)),
                &None,
            );
        }

        if self.dir_tree_visible {
            if let Some(path) = self.gallery.get_active_img_path() {
                if let Some(path) = tree::ui(path.to_str().unwrap_or(""), ctx) {
                    self.dir_tree_visible = false;
                    utils::set_mute_state(ctx, false);
                    self.new_images(&crawler::crawl(&path), &None);
                }
            }
        }

        if self.multi_gallery_visible {
            self.multi_gallery.ui(ctx, &mut self.gallery_selected_index);
            if let Some(img_name) = self.multi_gallery.selected_image_name() {
                self.gallery.select_by_name(img_name);
                self.multi_gallery_visible = false;
            }
        } else {
            self.gallery.ui(ctx);
        }

        self.calc_frametime();
    }
}

use crate::{
    callback::Callback,
    config::{Config, GeneralConfig},
    crawler,
    db::Db,
    metadata::Metadata,
    multi_gallery::MultiGallery,
    navigator,
    perf_metrics::PerfMetrics,
    single_gallery::SingleGallery,
    tree, utils, Order, VALID_EXTENSIONS,
};
use eframe::egui::{self, KeyboardShortcut};
use notify::{Event, INotifyWatcher, RecursiveMode, Watcher};
use rand::seq::SliceRandom;
use rand::thread_rng;
use rfd::FileDialog;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

pub struct App {
    paths: Vec<PathBuf>,
    gallery: SingleGallery,
    ///used when switching between galleries
    gallery_selected_index: Option<usize>,
    multi_gallery: MultiGallery,
    perf_metrics_visible: bool,
    multi_gallery_visible: bool,
    top_menu_visible: bool,
    dir_tree_visible: bool,
    base_path: PathBuf,
    dir_flattened: bool, //Fetches images for all subdirectories recursively
    navigator_visible: bool,
    navigator_search: String, //TODO: Investigate why this exists in the app struct
    perf_metrics: PerfMetrics,
    config: GeneralConfig,
    order: Order,
    watcher: Option<INotifyWatcher>,
    watcher_events: Arc<Mutex<Vec<Event>>>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::theme::apply_theme(&cc.egui_ctx);
        let cfg = Config::new();

        let mut style = (*cc.egui_ctx.style()).clone();

        for t_styles in style.text_styles.iter_mut() {
            t_styles.1.size *= cfg.general.text_scaling;
        }

        cc.egui_ctx.set_style(style);

        let (mut img_paths, opened_img_path) = crawler::paths_from_args();

        //TODO: Implement a default ordering
        Self::sort_images(&mut img_paths, &Order::DateDesc);
        img_paths.sort();

        match Db::init_db() {
            Ok(_) => {
                println!("Database initiated successfully");
                match Db::trim_db(&cfg.general.limit_cached) {
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
                &cfg.general.output_icc_profile,
            ),
            gallery_selected_index: None,
            multi_gallery: MultiGallery::new(
                &img_paths,
                cfg.multi_gallery,
                &cfg.general.output_icc_profile,
            ),
            perf_metrics_visible: false,
            multi_gallery_visible: false,
            top_menu_visible: false,
            dir_tree_visible: false,
            dir_flattened: false,
            base_path: Self::get_base_path(&img_paths),
            navigator_visible: false,
            navigator_search: Self::get_base_path(&img_paths)
                .to_str()
                .unwrap_or_default()
                .to_string(),
            perf_metrics: PerfMetrics::new(),
            config: cfg.general,
            order: Order::Asc,
            paths: img_paths,
            watcher: None,
            watcher_events: Arc::new(Mutex::new(vec![])),
        }
    }

    ///Returns the path to the opened image directory, if it's not unable to do this it then
    ///tries to return the users home, if this fail it just returns a default PathBuf
    fn get_base_path(paths: &[PathBuf]) -> PathBuf {
        if let Some(first_path) = paths.get(0) {
            if let Some(parent) = first_path.parent() {
                return parent.to_path_buf();
            }
        }

        if let Some(user_dirs) = directories::UserDirs::new() {
            return user_dirs.home_dir().to_path_buf();
        }

        PathBuf::default()
    }

    //Maybe have gallery show this
    fn handle_input(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_exit.kbd_shortcut)) {
            std::process::exit(0);
        }

        if utils::are_inputs_muted(ctx) {
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F10)) {
            self.perf_metrics_visible = !self.perf_metrics_visible;
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_menu.kbd_shortcut)) {
            self.top_menu_visible = !self.top_menu_visible;
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_toggle_gallery.kbd_shortcut)) {
            self.multi_gallery_visible = !self.multi_gallery_visible;
            self.gallery_selected_index = Some(self.gallery.selected_img_index);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_flatten_dir.kbd_shortcut)) {
            self.flatten_open_dir();
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_watcher_enabled.kbd_shortcut)) {
            self.enable_watcher();
        }
    }

    //Muter inputs will block all other inputs
    //This is required so typing in text boxes and the like doesn't
    //trigger shortcuts
    fn handle_input_muters(&mut self, ctx: &egui::Context) {
        let to_check: Vec<(&mut bool, &KeyboardShortcut)> = vec![
            (
                &mut self.navigator_visible,
                &self.config.sc_navigator.kbd_shortcut,
            ),
            (
                &mut self.dir_tree_visible,
                &self.config.sc_dir_tree.kbd_shortcut,
            ),
        ];

        let is_muted = utils::are_inputs_muted(ctx);

        //Assumes all muters can and will be closed with Escape
        for (active, shortcut) in to_check {
            if (is_muted && *active && ctx.input_mut(|i| i.consume_shortcut(shortcut)))
                || (!is_muted && ctx.input_mut(|i| i.consume_shortcut(shortcut)))
                || (*active && ctx.input(|i| i.key_pressed(egui::Key::Escape)))
            {
                *active = !*active;

                if *active {
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
            self.set_images_from_path(&folder, &None)
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
                self.set_images_from_path(parent, &Some(files[0].clone()))
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

    //Will crawl, assumes new directory
    fn set_images_from_path(&mut self, path: &Path, selected_img: &Option<PathBuf>) {
        self.paths = crawler::crawl(path, self.dir_flattened);
        Self::sort_images(&mut self.paths, &self.order);
        self.set_images(selected_img);
    }

    fn set_images_from_path_reset_index(&mut self, path: &Path, requires_new: bool) {
        let new_paths = crawler::crawl(path, self.dir_flattened);

        if requires_new && new_paths.iter().all(|x| self.paths.contains(x)) {
            return;
        }
        println!("new file in dir, loading");
        self.paths = new_paths;
        Self::sort_images(&mut self.paths, &self.order);
        self.set_images(&Some(self.paths[0].clone()));
    }

    fn set_images(&mut self, selected_img: &Option<PathBuf>) {
        Metadata::cache_metadata_for_images(&self.paths);
        self.load_images(selected_img, true);
    }

    fn load_images(&mut self, selected_img: &Option<PathBuf>, new_dir_opened: bool) {
        self.gallery.set_images(&self.paths, selected_img);
        self.multi_gallery.set_images(&self.paths);

        if new_dir_opened {
            self.base_path = Self::get_base_path(&self.paths);
            self.navigator_search = self.base_path.to_str().unwrap_or_default().to_string();
        }
    }

    //TODO: Fix order by date/etc/validate everything
    fn sort_images(paths: &mut [PathBuf], order: &Order) {
        println!("sorting images...");
        for path in paths.iter() {
            println!("{}", path.display());
        }

        if order == &Order::Random {
            paths.shuffle(&mut thread_rng());
        } else if order == &Order::Asc || order == &Order::Desc {
            paths.sort_by(|a, b| {
                let first: String;
                let second: String;

                if order == &Order::Asc {
                    first = a.file_name().unwrap().to_string_lossy().to_string();
                    second = b.file_name().unwrap().to_string_lossy().to_string();
                } else {
                    first = b.file_name().unwrap().to_string_lossy().to_string();
                    second = a.file_name().unwrap().to_string_lossy().to_string();
                }

                first.cmp(&second)
            });
        } else if order == &Order::DateAsc || order == &Order::DateDesc {
            paths.sort_by(|a, b| {
                let first: SystemTime;
                let second: SystemTime;

                if order == &Order::DateAsc {
                    first = a.metadata().unwrap().modified().unwrap();
                    second = b.metadata().unwrap().modified().unwrap();
                } else {
                    first = b.metadata().unwrap().modified().unwrap();
                    second = a.metadata().unwrap().modified().unwrap();
                }

                first
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .cmp(
                        &second
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    )
            });
        }

        for path in paths.iter() {
            println!("{}", path.display());
        }
    }

    fn enable_watcher(&mut self) {
        //TODO
        /*if self.dir_flattened {
            println!("Cannot enable watcher when dir is flattened");
        }*/

        println!("Enabling watcher at {:?}", self.base_path);

        let watcher_events = self.watcher_events.clone();
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    if let Ok(mut lock) = watcher_events.try_lock() {
                        lock.push(event.clone());
                    } else {
                        println!("Failure locking file watcher events queue");
                    }
                }
                Err(e) => println!("Error watching directory: {:?}", e),
            },
        )
        .unwrap();

        watcher
            .watch(&self.base_path, RecursiveMode::NonRecursive)
            .unwrap();

        self.watcher = Some(watcher);
    }

    fn process_file_watcher_events(&mut self) {
        self.set_images_from_path_reset_index(&self.base_path.clone(), true);

        //Ignore when we can't lock the mutex, it'll try next frame anyway
        if let Ok(mut events) = self.watcher_events.clone().try_lock() {
            if events.len() == 0 {
                return;
            }

            //New file could skew the order in whichever direction, so we just reload everything, fast either way
            if events.iter().any(|x| x.kind.is_create()) {
                self.set_images_from_path_reset_index(&self.base_path.clone(), true);
                return;
            }

            for event in events.iter().filter(|x| x.kind.is_modify()) {
                self.reload_galleries_image(Some(event.paths.first().unwrap().clone()));
            }

            events.clear();
        }
    }

    //TODO: When flattening cancel watcher and vice versa
    fn flatten_open_dir(&mut self) {
        println!("Flattening open directory");

        self.dir_flattened = true;
        self.set_images_from_path(&self.base_path.clone(), &self.gallery.get_active_img_path());
    }

    //Some callbacks affect both collections so it's important
    //to deal them in the base of the app
    fn execute_callback(&mut self, callback: Callback) {
        println!("Executing callback with {:?}", callback);
        match callback {
            Callback::Pop(path) => self.callback_pop(path),
            Callback::Reload(path) => self.reload_galleries_image(path),
            Callback::ReloadAll => self.callback_reload_all(),
            Callback::NoAction => {}
        }
    }

    fn callback_pop(&mut self, path: Option<PathBuf>) {
        if let Some(path) = path {
            self.gallery.pop(&path);
            self.multi_gallery.pop(&path);
        }
    }

    fn reload_galleries_image(&mut self, path: Option<PathBuf>) {
        if let Some(path) = path {
            self.gallery.reload_at(&path);
            self.multi_gallery.reload_at(&path);
        }
    }

    fn callback_reload_all(&mut self) {
        self.set_images_from_path(&self.base_path.clone(), &self.gallery.get_active_img_path());
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        let mut order_changed = false;

        self.perf_metrics.new_frame();
        self.handle_input_muters(ctx);
        self.handle_input(ctx);

        egui::TopBottomPanel::top("performance_metrics")
            .show_separator_line(false)
            .show_animated(ctx, self.perf_metrics_visible, |ui| {
                self.perf_metrics.display_metrics(ui);
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
            self.set_images_from_path(&PathBuf::from(self.navigator_search.clone()), &None);
        }

        if self.dir_tree_visible {
            if let Some(path) = self.gallery.get_active_img_path() {
                if let Some(path) = tree::ui(path.to_str().unwrap_or(""), ctx) {
                    self.dir_tree_visible = false;
                    utils::set_mute_state(ctx, false);
                    self.set_images_from_path(&path, &None);
                }
            }
        }

        if self.multi_gallery_visible {
            self.multi_gallery.ui(ctx, &mut self.gallery_selected_index);

            if let Some(img_name) = self.multi_gallery.selected_image_name() {
                self.gallery.select_by_name(img_name);
                self.multi_gallery_visible = false;
            }

            if let Some(callback) = self.multi_gallery.take_callback() {
                self.execute_callback(callback);
            }
        } else {
            self.gallery.ui(ctx, &mut self.order, &mut order_changed);

            if let Some(callback) = self.gallery.take_callback() {
                self.execute_callback(callback);
            }
        }

        if order_changed {
            self.set_images_from_path_reset_index(&self.base_path.clone(), false);
        }

        self.process_file_watcher_events();

        self.perf_metrics.end_frame();
    }
}

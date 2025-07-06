use crate::filters::Filters;
use crate::{
    callback::Callback,
    config::{Config, GeneralConfig},
    crawler,
    db::Db,
    grid_view::GridView,
    image_view::ImageView,
    metadata::Metadata,
    navigator,
    perf_metrics::PerfMetrics,
    tree, utils, VALID_EXTENSIONS,
};
use eframe::egui::{self, KeyboardShortcut, RichText};
use notify::{Event, INotifyWatcher, RecursiveMode, Watcher};
use rfd::FileDialog;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub struct App {
    paths: Vec<PathBuf>,
    gallery: ImageView,
    ///used when switching between view modes
    gallery_selected_index: Option<usize>,
    grid_view: GridView,
    perf_metrics_visible: bool,
    grid_view_visible: bool,
    top_menu_visible: bool,
    dir_tree_visible: bool,
    base_path: PathBuf,
    dir_flattened: bool, //Fetches images for all subdirectories recursively
    navigator_visible: bool,
    navigator_search: String, //TODO: Investigate why this exists in the app struct
    perf_metrics: PerfMetrics,
    config: GeneralConfig,
    watcher: Option<INotifyWatcher>,
    watcher_events: Arc<Mutex<Vec<Event>>>,
    filters: Filters,
    side_panel_visible: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let cfg = Config::new();

        crate::theme::apply_theme(&cc.egui_ctx);
        let mut style = (*cc.egui_ctx.style()).clone();

        for t_styles in style.text_styles.iter_mut() {
            t_styles.1.size *= cfg.general.text_scaling;
        }

        cc.egui_ctx.set_style(style);

        let (mut img_paths, opened_img_path) = crawler::paths_from_args();

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

        let base_path = Self::get_base_path(&img_paths);
        Self {
            gallery: ImageView::new(
                &img_paths,
                &opened_img_path,
                cfg.image_view,
                &cfg.general.output_icc_profile,
            ),
            gallery_selected_index: None,
            grid_view: GridView::new(&img_paths, cfg.grid_view, &cfg.general.output_icc_profile),
            perf_metrics_visible: false,
            grid_view_visible: false,
            top_menu_visible: false,
            dir_tree_visible: false,
            side_panel_visible: false,
            dir_flattened: false,
            base_path: base_path.clone(),
            navigator_visible: false,
            navigator_search: base_path.to_str().unwrap_or_default().to_string(),
            perf_metrics: PerfMetrics::new(),
            config: cfg.general,
            filters: Filters::new(cfg.filter, base_path.to_str().unwrap_or("")),
            paths: img_paths,
            watcher: None,
            watcher_events: Arc::new(Mutex::new(vec![])),
        }
    }

    ///Returns the path to the opened image directory if it's not unable to do this, it then
    ///tries to return the users home, if this fails, it just returns a default PathBuf
    fn get_base_path(paths: &[PathBuf]) -> PathBuf {
        if let Some(first_path) = paths.first() {
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
            self.grid_view_visible = !self.grid_view_visible;
            self.gallery_selected_index = Some(self.gallery.selected_img_index);
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_flatten_dir.kbd_shortcut)) {
            self.flatten_open_dir();
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_watch_directory.kbd_shortcut)) {
            self.enable_watcher();
        }

        if ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_toggle_side_panel.kbd_shortcut)) {
            self.side_panel_visible = !self.side_panel_visible;
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
        self.set_images(selected_img, true);
    }

    fn set_images_from_paths(&mut self, paths: Vec<PathBuf>) {
        self.paths = paths;
        self.set_images(&None, true);
    }

    fn set_images(&mut self, selected_img: &Option<PathBuf>, new_dir_opened: bool) {
        Metadata::cache_metadata_for_images(&self.paths);
        self.load_images(selected_img, new_dir_opened);
    }

    fn load_images(&mut self, selected_img: &Option<PathBuf>, new_dir_opened: bool) {
        self.gallery.set_images(&self.paths, selected_img);
        self.grid_view.set_images(&self.paths);

        if new_dir_opened {
            self.base_path = Self::get_base_path(&self.paths);
            self.navigator_search = self.base_path.to_str().unwrap_or_default().to_string();
        }
    }

    fn enable_watcher(&mut self) {
        if self.watcher.is_some() {
            println!("Disabling watcher");
            self.watcher = None;
            return;
        }

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

        //Can be expensive on trees with a lot of files, but it's up to the user.
        let recursive_mode = if self.dir_flattened {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher.watch(&self.base_path, recursive_mode).unwrap();

        self.watcher = Some(watcher);
    }

    fn process_file_watcher_events(&mut self) {
        //Ignore when we can't lock the mutex, it'll try next frame anyway
        if let Ok(mut events) = self.watcher_events.clone().try_lock() {
            if events.len() == 0 {
                return;
            }

            let mut should_reload = false;
            let mut selected_img_path = None;
            for event in events.iter() {
                let mut event_paths = event.paths.clone();

                event_paths.reverse();

                let first = event_paths.first().unwrap();

                if utils::is_invalid_file(first) {
                    continue;
                }

                if event.kind.is_modify() {
                    if self.paths.contains(first) {
                        self.reload_galleries_image(Some(first.clone()));
                    } else {
                        self.paths.push(first.clone());
                        selected_img_path = Some(first.clone());
                        should_reload = true;
                    }
                } else if event.kind.is_create() {
                    self.paths.push(first.clone());
                    selected_img_path = Some(first.clone());
                    should_reload = true;
                }
            }

            if should_reload {
                self.set_images(&selected_img_path, false);
            }

            events.clear();
        }
    }

    fn flatten_open_dir(&mut self) {
        if self.dir_flattened {
            println!("Returning to original directory");
            self.dir_flattened = false;

            //restart watcher in non-recursive mode
            if self.watcher.is_some() {
                self.watcher = None;
                self.enable_watcher();
            }

            self.set_images_from_path(&self.base_path.clone(), &None);
        } else {
            println!("Flattening open directory");
            self.dir_flattened = true;

            //restart watcher in recursive mode
            if self.watcher.is_some() {
                self.watcher = None;
                self.enable_watcher();
            }

            self.set_images_from_path(&self.base_path.clone(), &self.gallery.get_active_img_path());
        }
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
            self.grid_view.pop(&path);
        }
    }

    fn reload_galleries_image(&mut self, path: Option<PathBuf>) {
        if let Some(path) = path {
            self.gallery.reload_at(&path);
            self.grid_view.reload_at(&path);
        }
    }

    fn callback_reload_all(&mut self) {
        self.set_images_from_path(&self.base_path.clone(), &self.gallery.get_active_img_path());
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

        egui::SidePanel::right("image_metadata")
            .resizable(true)
            .show_separator_line(false)
            .min_width(200.)
            .show_animated(ctx, self.side_panel_visible, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(filtered_paths) = self.filters.ui(ui) {
                        self.set_images_from_paths(filtered_paths);
                    }
                    ui.add_space(20.);
                    ui.separator();
                    ui.add_space(10.);
                    ui.label(RichText::new("Image Metadata").heading());
                    ui.add_space(10.);
                    if let Some(selected_img) = self.gallery.get_active_img_mut() {
                        selected_img.metadata_ui(ui, &self.config.metadata_tags);
                    }
                })
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

        if self.grid_view_visible {
            self.grid_view.ui(ctx, &mut self.gallery_selected_index);

            if let Some(img_name) = self.grid_view.selected_image_name() {
                self.gallery.select_by_name(img_name);
                self.grid_view_visible = false;
            }

            if let Some(callback) = self.grid_view.take_callback() {
                self.execute_callback(callback);
            }
        } else {
            self.gallery
                .ui(ctx, self.dir_flattened, self.watcher.is_some());

            if let Some(callback) = self.gallery.take_callback() {
                self.execute_callback(callback);
            }
        }

        if self.watcher.is_some() {
            ctx.request_repaint();
        }

        self.process_file_watcher_events();

        self.perf_metrics.end_frame();
    }
}

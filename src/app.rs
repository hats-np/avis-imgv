use crate::COLOR_GREY_DARK_BG;
use crate::db::DbRepository;
use crate::filters::Filters;
use crate::image_store::ImageStore;
use crate::metadata::Metadata;
use crate::worker::{PostProcessingMessage, Worker};
use crate::{
    VALID_EXTENSIONS,
    callback::Callback,
    config::{Config, GeneralConfig},
    crawler,
    grid_view::GridView,
    image_view::ImageView,
    navigator,
    perf_metrics::PerfMetrics,
    tree, utils,
};
use eframe::Frame;
use eframe::egui::{self, KeyboardShortcut, Panel, RichText, Ui, ViewportCommand, Window, frame};
use epaint::Stroke;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use notify::FsEventWatcher;
#[cfg(any(target_os = "linux", target_os = "android"))]
use notify::INotifyWatcher;
use notify::{RecursiveMode, Watcher};
use rfd::FileDialog;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub struct App {
    paths: Vec<PathBuf>,
    gallery: ImageView,
    grid_view: GridView,
    base_path: PathBuf,
    dir_flattened: bool,      //Fetches images for all subdirectories recursively
    navigator_search: String, //TODO: Investigate why this exists in the app struct
    perf_metrics: PerfMetrics,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    watcher: Option<INotifyWatcher>,
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    watcher: Option<FsEventWatcher>,
    watcher_events: Option<Receiver<notify::Event>>,
    filters: Filters,
    worker: Arc<Mutex<Worker>>,
    fullscreen: bool,
    state: AppState,
}

//The state is seperate from the App struct
//Since its reference is meant to be passed into views
//And other elements.
pub struct AppState {
    pub view_visibility: AppViewVisibility,
    pub selected_img_index: usize,
    pub selected_img_indexes: Vec<usize>,
    pub selected_img_index_changed: bool,
    pub image_store: ImageStore,
    pub thumbnail_store: ImageStore,
    pub general_config: GeneralConfig,
    pub grouping: AppFileGrouping,
}

pub struct AppFileGrouping {
    pub enabled: bool,
    pub lookup: HashMap<String, Vec<PathBuf>>,
}

pub struct AppViewVisibility {
    pub perf_metrics: bool,
    pub grid_view: bool,
    pub top_menu: bool,
    pub dir_tree: bool,
    pub navigator: bool,
    pub side_panel: bool,
}

impl AppState {
    pub fn get_active_img_nr(&mut self) -> usize {
        self.selected_img_index + 1
    }

    pub fn set_selected_img_index(&mut self, i: usize, trigger_change: bool) {
        self.selected_img_index = i;
        if trigger_change {
            self.selected_img_index_changed = trigger_change;
            self.selected_img_indexes = vec![i];
        }
    }

    pub fn set_selected_img_indexes(&mut self, end: usize) {
        self.selected_img_indexes = (self.selected_img_index..end + 1).collect();
    }

    pub fn append_selected_img_index(&mut self, i: usize) {
        if self.selected_img_indexes.contains(&i) {
            self.selected_img_indexes.retain(|&x| x != i);
        } else {
            self.selected_img_indexes.push(i);
        }
    }

    pub fn is_image_selected(&self, i: &usize) -> bool {
        self.selected_img_indexes.contains(i)
    }

    pub fn reset_selection_to_current_img(&mut self) {
        self.selected_img_indexes = vec![];
    }

    pub fn select_current_img(&mut self) {
        if !self.selected_img_indexes.contains(&self.selected_img_index) {
            self.selected_img_indexes.push(self.selected_img_index);
        } else {
            self.selected_img_indexes
                .retain(|&x| x != self.selected_img_index);
        }
    }

    pub fn select_all(&mut self, nr_of_imgs: usize) {
        self.selected_img_indexes = (0..nr_of_imgs).collect();
    }

    pub fn get_grouped_img_paths(
        &self,
        path: &Path,
        include_non_raw: bool,
    ) -> Option<Vec<PathBuf>> {
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if let Some(raws) = self.grouping.lookup.get(&stem) {
            let mut raws = raws.clone();
            if include_non_raw {
                raws.push(path.to_path_buf());
                raws.reverse(); //This way our preview image is always the first on the vec
            }
            Some(raws)
        } else {
            None
        }
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, slideshow: bool, fullscreen: bool) -> Self {
        let cfg = Config::new();

        crate::theme::apply_theme(&cc.egui_ctx);
        let mut style = (*cc.egui_ctx.global_style()).clone();

        for t_styles in style.text_styles.iter_mut() {
            t_styles.1.size *= cfg.general.text_scaling;
        }

        cc.egui_ctx.set_global_style(style);

        if fullscreen {
            cc.egui_ctx
                .send_viewport_cmd(ViewportCommand::Fullscreen(true));
        }

        let (mut img_paths, opened_img_path) = crawler::paths_from_args();

        img_paths.sort();

        let mut db_repo = DbRepository::new();
        let worker = Worker::new(cc.egui_ctx.clone(), &db_repo);

        match db_repo.init_db() {
            Ok(_) => {
                tracing::info!("Database initiated successfully");
                match db_repo.trim_db(&cfg.general.limit_cached) {
                    Ok(_) => worker.send_job(crate::worker::Job::CacheMetadataForImages(
                        img_paths.clone(),
                    )),
                    Err(e) => {
                        tracing::info!("Failure trimming db {e}");
                    }
                };
            }
            Err(e) => {
                tracing::info!("Failure initiating db -> {e}");
            }
        };

        let grouping = if cfg.general.group_raw_pairs {
            let mut grouping = AppFileGrouping {
                enabled: true,
                lookup: HashMap::new(),
            };

            let count_before = img_paths.len();
            img_paths = Metadata::group_raw_pairs(&img_paths, &mut grouping.lookup);

            tracing::info!(
                "Grouped {} images into {} groups based on raw pairs",
                count_before - img_paths.len(),
                grouping.lookup.len()
            );

            grouping
        } else {
            AppFileGrouping {
                enabled: false,
                lookup: HashMap::new(),
            }
        };

        let render_state = match cc.wgpu_render_state.clone() {
            Some(rs) => rs,
            None => panic!("Failure fetching render state at startup. Startup cannot proceed"),
        };

        let max_texture_size = render_state.adapter.limits().max_texture_dimension_2d;

        let base_path = Self::get_base_path(&img_paths, &opened_img_path);
        let worker = Arc::new(Mutex::new(worker));
        let image_store = ImageStore::new(
            cfg.general.output_icc_profile.to_owned(),
            max_texture_size,
            &render_state,
            &db_repo,
            cfg.general.simultaneous_load,
            &cfg.general.raw_exiftool_preview_ext,
        );
        let thumbnail_store = ImageStore::new(
            cfg.general.output_icc_profile.to_owned(),
            max_texture_size,
            &render_state,
            &db_repo,
            cfg.general.simultaneous_load,
            &cfg.general.raw_exiftool_preview_ext,
        );
        let mut state = AppState {
            image_store,
            thumbnail_store,
            view_visibility: AppViewVisibility {
                perf_metrics: false,
                grid_view: false,
                top_menu: false,
                dir_tree: false,
                navigator: false,
                side_panel: false,
            },
            selected_img_index: 0,
            selected_img_indexes: vec![],
            selected_img_index_changed: false,
            general_config: cfg.general,
            grouping,
        };
        Self {
            gallery: ImageView::new(
                &img_paths,
                &opened_img_path,
                cfg.image_view,
                slideshow,
                cfg.slideshow,
                &mut state,
            ),
            grid_view: GridView::new(&img_paths, cfg.grid_view),
            base_path: base_path.clone(),
            navigator_search: base_path.to_str().unwrap_or_default().to_string(),
            perf_metrics: PerfMetrics::new(),
            filters: Filters::new(cfg.filter, &base_path, worker.clone(), &db_repo),
            paths: img_paths,
            watcher: None,
            watcher_events: None,
            worker,
            fullscreen,
            dir_flattened: false,
            state,
        }
    }

    ///Returns the path to the opened image directory if it's not unable to do this, it then
    ///tries to return the users home, if this fails, it just returns a default PathBuf
    fn get_base_path(paths: &[PathBuf], opened_img_path: &Option<PathBuf>) -> PathBuf {
        if let Some(opened_img_path) = opened_img_path {
            return opened_img_path.clone();
        }

        if let Some(first_path) = paths.first()
            && let Some(parent) = first_path.parent()
        {
            return parent.to_path_buf();
        }

        if let Some(user_dirs) = directories::UserDirs::new() {
            tracing::info!("Failure fetching opened path, using users home");
            return user_dirs.home_dir().to_path_buf();
        }

        PathBuf::default()
    }

    //Maybe have gallery show this
    fn handle_input(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_shortcut(&self.state.general_config.sc_exit.kbd_shortcut)) {
            std::process::exit(0);
        }

        if utils::are_inputs_muted(ctx) {
            return;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F10)) {
            self.state.view_visibility.perf_metrics = !self.state.view_visibility.perf_metrics;
        }

        ctx.input_mut(|i| {
            if i.consume_shortcut(&self.state.general_config.sc_toggle_side_panel.kbd_shortcut) {
                self.state.view_visibility.side_panel = !self.state.view_visibility.side_panel;
            }

            if i.consume_shortcut(&self.state.general_config.sc_watch_directory.kbd_shortcut) {
                self.enable_watcher();
            }

            if i.consume_shortcut(&self.state.general_config.sc_flatten_dir.kbd_shortcut) {
                self.flatten_open_dir();
            }

            if i.consume_shortcut(&self.state.general_config.sc_toggle_gallery.kbd_shortcut) {
                self.state.view_visibility.grid_view = !self.state.view_visibility.grid_view;
                self.grid_view.jump_to_index = Some(self.state.selected_img_index)
            }

            if i.consume_shortcut(&self.state.general_config.sc_menu.kbd_shortcut) {
                self.state.view_visibility.top_menu = !self.state.view_visibility.top_menu;
            }

            if i.consume_shortcut(&self.state.general_config.sc_rate_0.kbd_shortcut) {
                self.launch_rate_xmp_job(0);
            }

            if i.consume_shortcut(&self.state.general_config.sc_rate_reject.kbd_shortcut) {
                self.launch_rate_xmp_job(-1);
            }

            if i.consume_shortcut(&self.state.general_config.sc_rate_1.kbd_shortcut) {
                self.launch_rate_xmp_job(1);
            }

            if i.consume_shortcut(&self.state.general_config.sc_rate_2.kbd_shortcut) {
                self.launch_rate_xmp_job(2);
            }

            if i.consume_shortcut(&self.state.general_config.sc_rate_3.kbd_shortcut) {
                self.launch_rate_xmp_job(3);
            }

            if i.consume_shortcut(&self.state.general_config.sc_rate_4.kbd_shortcut) {
                self.launch_rate_xmp_job(4);
            }

            if i.consume_shortcut(&self.state.general_config.sc_rate_5.kbd_shortcut) {
                self.launch_rate_xmp_job(5);
            }

            if i.consume_shortcut(&self.state.general_config.sc_reset_selection.kbd_shortcut) {
                self.state.reset_selection_to_current_img();
            }

            if i.consume_shortcut(&self.state.general_config.sc_select_current_img.kbd_shortcut) {
                self.state.select_current_img();
            }

            if i.consume_shortcut(&self.state.general_config.sc_select_all.kbd_shortcut) {
                self.state.select_all(self.paths.len());
            }
        });

        if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
            self.fullscreen = true;
        }
    }

    //Muter inputs will block all other inputs
    //This is required so typing in text boxes and the like doesn't
    //trigger shortcuts
    fn handle_input_muters(&mut self, ctx: &egui::Context) {
        let to_check: Vec<(&mut bool, &KeyboardShortcut)> = vec![
            (
                &mut self.state.view_visibility.navigator,
                &self.state.general_config.sc_navigator.kbd_shortcut,
            ),
            (
                &mut self.state.view_visibility.dir_tree,
                &self.state.general_config.sc_dir_tree.kbd_shortcut,
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

        if let Some(files) = files
            && let Some(parent) = &files[0].parent()
        {
            self.set_images_from_path(parent, &Some(files[0].clone()))
        }
    }

    fn get_file_dialog(&mut self) -> FileDialog {
        let mut file_dialog = FileDialog::new();

        if let Some(path) = self.gallery.get_active_img_path(&self.state)
            && let Some(parent) = path.parent()
        {
            file_dialog = file_dialog.set_directory(parent);
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
        self.set_images(&None, false);
    }

    fn set_images(&mut self, selected_img: &Option<PathBuf>, new_dir_opened: bool) {
        if let Ok(worker) = self.worker.try_lock() {
            worker.send_job(crate::worker::Job::CacheMetadataForImages(
                self.paths.clone(),
            ));
        } else {
            tracing::error!("Failure locking mutex for metadata cache job");
        }
        self.load_images(selected_img, new_dir_opened);
    }

    fn load_images(&mut self, selected_img: &Option<PathBuf>, new_dir_opened: bool) {
        self.gallery
            .set_images(&self.paths, selected_img, &mut self.state);
        self.grid_view.set_images(&self.paths, &mut self.state);

        if new_dir_opened {
            self.base_path = Self::get_base_path(&self.paths, &None);
            self.filters.set_metadata_directory_value(&self.base_path);
            self.navigator_search = self.base_path.to_str().unwrap_or_default().to_string();
        }
    }

    fn enable_watcher(&mut self) {
        if self.watcher.is_some() {
            tracing::info!("Disabling watcher");
            self.watcher = None;
            return;
        }

        tracing::info!("Enabling watcher at {:?}", self.base_path);

        let (tx, rx) = channel();
        self.watcher_events = Some(rx);
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| match res {
                Ok(event) => {
                    let _ = tx.send(event);
                }
                Err(e) => tracing::info!("Error watching directory: {e:?}"),
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
        if self.watcher_events.is_none() {
            return;
        }

        let mut paths: Vec<PathBuf> = vec![];
        if let Some(receiver) = &self.watcher_events {
            let mut should_reload = false;
            let mut selected_img_path = None;
            while let Ok(event) = receiver.try_recv() {
                let mut event_paths = event.paths.clone();

                event_paths.reverse();

                let first = event_paths.first().unwrap();

                if utils::is_invalid_file(first) {
                    continue;
                }

                if (event.kind.is_modify() || event.kind.is_access() || event.kind.is_create())
                    && !paths.contains(first)
                {
                    paths.push(first.clone());
                }
            }

            for p in paths {
                if self.paths.contains(&p) {
                    self.reload_galleries_image(Some(p));
                } else {
                    self.paths.push(p.clone());
                    selected_img_path = Some(p);
                    should_reload = true;
                }
            }

            if should_reload {
                self.set_images(&selected_img_path, false);
            }
        }
    }

    fn flatten_open_dir(&mut self) {
        if self.dir_flattened {
            tracing::info!("Returning to original directory");
            self.dir_flattened = false;

            //restart watcher in non-recursive mode
            if self.watcher.is_some() {
                self.watcher = None;
                self.enable_watcher();
            }

            self.set_images_from_path(&self.base_path.clone(), &None);
        } else {
            tracing::info!("Flattening open directory: {:?}", &self.base_path);
            self.dir_flattened = true;

            //restart watcher in recursive mode
            if self.watcher.is_some() {
                self.watcher = None;
                self.enable_watcher();
            }

            self.set_images_from_path(
                &self.base_path.clone(),
                &self.gallery.get_active_img_path(&self.state),
            );
        }
    }

    //Some callbacks affect both collections so it's important
    //to deal them in the base of the app
    fn execute_callback(&mut self, callback: Callback) {
        tracing::info!("Executing callback with {callback:?}");
        match callback {
            Callback::Pop(path) => self.callback_pop(path),
            Callback::Reload(path) => self.reload_galleries_image(path),
            Callback::ReloadAll => self.callback_reload_all(),
            Callback::Advance => self.callback_advance(),
            Callback::NoAction => {}
            Callback::CloseView => {}
        }
    }

    fn callback_pop(&mut self, path: Option<PathBuf>) {
        if let Some(path) = path {
            self.gallery.pop(&path, &mut self.state);
            self.grid_view.pop(&path);
        }
    }

    fn callback_advance(&mut self) {
        self.gallery.next_image(&mut self.state);
    }

    fn reload_galleries_image(&mut self, path: Option<PathBuf>) {
        if let Some(path) = path {
            self.gallery.reload_at(&path, &mut self.state.image_store);
            self.grid_view
                .reload_at(&path, &mut self.state.thumbnail_store);
        }
    }

    fn callback_reload_all(&mut self) {
        self.set_images_from_path(
            &self.base_path.clone(),
            &self.gallery.get_active_img_path(&self.state),
        );
    }

    fn execute_img_store_routines(&mut self, ui: &mut Ui) {
        self.state.image_store.update();
        self.state.thumbnail_store.update();

        if self.state.image_store.has_any_imgs_loading()
            || self.state.thumbnail_store.has_any_imgs_loading()
        {
            ui.request_repaint();
        }
    }

    fn execute_worker_post_processing(&mut self) {
        let pp_messages = if let Ok(mut worker) = self.worker.try_lock() {
            worker.get_all_post_processing_messages()
        } else {
            return;
        };

        for pp in pp_messages {
            match pp {
                PostProcessingMessage::RefreshMetadata => {
                    self.gallery.reload_all_imgs(&mut self.state);
                    self.grid_view.reload_all_imgs(&mut self.state);
                    self.gallery.clear_img_display_names();
                }
                PostProcessingMessage::RefreshRating(paths, rating) => {
                    self.state
                        .image_store
                        .refresh_imgs_xmp_rating(&paths, rating);
                    self.state
                        .thumbnail_store
                        .refresh_imgs_xmp_rating(&paths, rating);
                    self.gallery.clear_img_display_names();
                }
            }
        }
    }

    fn show_worker_msg(&mut self, ui: &mut Ui) {
        let msg_to_display = if let Ok(mut worker) = self.worker.try_lock() {
            worker.get_latest_msg().clone()
        } else {
            return;
        };

        if let Some(msg) = msg_to_display {
            let _max_rect = ui.max_rect();

            Window::new("WorkerMsgWindow")
                .vscroll(false)
                .resizable(false)
                .title_bar(false)
                .movable(false)
                .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12., -40.))
                .frame(
                    frame::Frame::new()
                        .fill(COLOR_GREY_DARK_BG)
                        .multiply_with_opacity(1.)
                        .corner_radius(4.)
                        .inner_margin(5.),
                )
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(msg);
                    });
                });
        }
    }

    pub fn show_selected_img_counter(&mut self, ui: &mut Ui, margin_right: f32) {
        if !self.state.selected_img_indexes.is_empty() {
            let _max_rect = ui.max_rect();

            Window::new("SelectedImgCounter")
                .vscroll(false)
                .resizable(false)
                .title_bar(false)
                .movable(false)
                .anchor(
                    egui::Align2::RIGHT_BOTTOM,
                    egui::vec2(-12. - margin_right, -40.),
                )
                .frame(
                    frame::Frame::new()
                        .fill(COLOR_GREY_DARK_BG)
                        .stroke(Stroke::new(2.0, self.state.general_config.accent_color))
                        .multiply_with_opacity(1.)
                        .corner_radius(4.)
                        .inner_margin(5.),
                )
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{} Images selected",
                            self.state.selected_img_indexes.len()
                        ));
                    });
                });
        }
    }

    pub fn launch_rate_xmp_job(&mut self, rating: i32) {
        if let Ok(worker) = self.worker.try_lock() {
            worker.send_job(crate::worker::Job::SetXMPRating(
                self.get_selected_img_paths(),
                rating,
            ));
        } else {
            tracing::error!("Failure locking mutex for xmp rating job");
        }
    }

    //Clone doesn't matter only used for some user actions as of now
    //If used on something that runs every frame we need to think better
    //about this
    pub fn get_selected_img_paths(&self) -> Vec<PathBuf> {
        let mut selected_paths: Vec<PathBuf> = vec![];
        if self.state.selected_img_indexes.len() > 1
            || (self.state.selected_img_indexes.len() == 1
                && self.state.selected_img_indexes[0] != self.state.selected_img_index)
        {
            for i in &self.state.selected_img_indexes {
                selected_paths.push(self.paths[*i].clone());
            }
        } else {
            selected_paths = vec![self.paths[self.state.selected_img_index].clone()];
        };

        if self.state.grouping.enabled {
            let mut to_append: Vec<PathBuf> = vec![];
            for path in &selected_paths {
                if let Some(raws) = self.state.get_grouped_img_paths(path, false) {
                    for r in raws {
                        if !selected_paths.contains(&r) {
                            to_append.push(r.clone());
                        }
                    }
                }
            }

            selected_paths = [&selected_paths[..], &to_append[..]].concat();
        }

        selected_paths
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        self.perf_metrics.new_frame();
        self.execute_img_store_routines(ui);
        self.handle_input_muters(ui.ctx());
        self.handle_input(ui.ctx());

        Panel::top("performance_metrics")
            .show_separator_line(false)
            .show_animated_inside(ui, self.state.view_visibility.perf_metrics, |ui| {
                self.perf_metrics.display_metrics(ui);
                ui.ctx().clone().texture_ui(ui);
            });

        Panel::top("menu")
            .show_separator_line(false)
            .show_animated_inside(ui, self.state.view_visibility.top_menu, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open Folder").clicked() {
                        self.folder_picker();
                        ui.close();
                    }

                    if ui.button("Open Files").clicked() {
                        self.files_picker();
                        ui.close();
                    }
                });
            });

        let image_metadata_panel = Panel::right("image_metadata")
            .resizable(true)
            .show_separator_line(false)
            .show_animated_inside(ui, self.state.view_visibility.side_panel, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if let Some(filtered_paths) = self.filters.ui(ui, &mut self.state) {
                        self.set_images_from_paths(filtered_paths);
                    }
                    ui.add_space(20.);
                    ui.separator();
                    ui.add_space(10.);
                    ui.label(RichText::new("Image Metadata").heading());
                    ui.add_space(10.);
                    if let Some(selected_img) = self.gallery.get_active_img_mut(&self.state) {
                        selected_img.metadata_ui(
                            ui,
                            &self.state.general_config.metadata_tags,
                            &self.state.image_store,
                        );
                    }
                })
            });

        if self.state.view_visibility.navigator
            && navigator::ui(&mut self.navigator_search, ui.ctx())
        {
            self.state.view_visibility.navigator = false;
            utils::set_mute_state(ui.ctx(), false);
            self.set_images_from_path(&PathBuf::from(self.navigator_search.clone()), &None);
        }

        if self.state.view_visibility.dir_tree
            && let Some(path) = self.gallery.get_active_img_path(&self.state)
            && let Some(path) = tree::ui(path.to_str().unwrap_or(""), ui.ctx())
        {
            self.state.view_visibility.dir_tree = false;
            utils::set_mute_state(ui.ctx(), false);
            self.set_images_from_path(&path, &None);
        }

        if self.state.view_visibility.grid_view {
            self.grid_view.ui(ui, &mut self.state);

            if self.state.selected_img_index_changed {
                self.gallery.load(&mut self.state);
            }

            if let Some(callback) = self.grid_view.take_callback() {
                if callback == Callback::CloseView {
                    self.state.view_visibility.grid_view = false;
                } else {
                    self.execute_callback(callback);
                }
            }
        } else {
            self.gallery.ui(
                ui,
                self.dir_flattened,
                self.watcher.is_some(),
                &mut self.state,
            );

            if let Some(callback) = self.gallery.take_callback() {
                self.execute_callback(callback);
            }
        }

        self.show_worker_msg(ui);
        self.show_selected_img_counter(
            ui,
            if let Some(r) = image_metadata_panel {
                r.response.rect.width() + 12.
            } else {
                0.
            },
        );

        if self.watcher.is_some() {
            ui.ctx().request_repaint();
        }

        self.process_file_watcher_events();
        self.execute_worker_post_processing();
        self.perf_metrics.end_frame();
    }
}

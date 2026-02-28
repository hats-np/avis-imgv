use crate::{
    callback::Callback, config::GridViewConfig, image_store::ImageStore,
    thumbnail_image::ThumbnailImage, user_action::show_context_menu, utils,
};
use eframe::{
    egui::{self, Ui, scroll_area::ScrollSource},
    epaint::Vec2,
};
use std::path::{Path, PathBuf};

pub struct GridView {
    imgs: Vec<ThumbnailImage>,
    config: GridViewConfig,
    selected_image_name: Option<String>,
    prev_img_size: f32,
    prev_scroll_offset: f32,
    total_rows: usize,
    images_per_row: usize,
    prev_images_per_row: usize,
    prev_row_range_start: usize,
    reset_scroll: bool,
    callback: Option<Callback>,
}

impl GridView {
    pub fn new(image_paths: &[PathBuf], config: GridViewConfig) -> GridView {
        let imgs = ThumbnailImage::from_paths(image_paths);
        let mut mg = GridView {
            total_rows: 0,
            imgs,
            selected_image_name: None,
            images_per_row: config.images_per_row,
            prev_images_per_row: config.images_per_row,
            config,
            prev_img_size: 0.,
            prev_scroll_offset: 0.,
            prev_row_range_start: 0,
            reset_scroll: false,
            callback: None,
        };

        mg.set_total_rows();

        mg
    }

    pub fn set_images(&mut self, img_paths: &[PathBuf], image_store: &mut ImageStore) {
        for img in self.imgs.iter().filter(|x| x.registered) {
            image_store.deregister_img(&img.path);
        }
        self.imgs = ThumbnailImage::from_paths(img_paths);
        self.reset_scroll = true;
        self.set_total_rows();
    }

    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        jump_to_index: &mut Option<usize>,
        image_store: &mut ImageStore,
    ) {
        self.handle_input(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0., 0.);
            ui.set_min_width(ui.available_width());

            let mut img_size = ui.available_width() / self.images_per_row as f32;
            let prev_img_size = img_size;

            if img_size % 6. != 0. {
                img_size -= img_size % 6.; // Truncate to nearest multiple of 6
            }

            let remainder = (prev_img_size - img_size) * self.images_per_row as f32;

            let mut scroll_area = egui::ScrollArea::vertical().scroll_source(ScrollSource::ALL);

            //Since image size changes when we resize the window, we need to compensate the scroll
            //offset as show_rows assumes fixed widget sizes
            if img_size != self.prev_img_size {
                scroll_area = scroll_area.scroll_offset(Vec2 {
                    x: 0.,
                    y: img_size * self.prev_scroll_offset / self.prev_img_size,
                });
            }

            if self.images_per_row != self.prev_images_per_row {
                let target_row =
                    (self.prev_row_range_start * self.prev_images_per_row) / self.images_per_row;

                scroll_area = scroll_area.scroll_offset(Vec2 {
                    x: 0.,
                    y: img_size * target_row as f32,
                });
            }

            if let Some(mut i) = jump_to_index.take() {
                //Get start of the row index so it's easier to calculate the offset
                i = i - (i % self.images_per_row);
                let scroll_offset = ((i as f32) / self.images_per_row as f32) * img_size;
                scroll_area = scroll_area.scroll_offset(Vec2 {
                    x: 0.,
                    y: scroll_offset,
                })
            };

            if self.reset_scroll {
                scroll_area = scroll_area.scroll_offset(Vec2 { x: 0., y: 0. });
                self.reset_scroll = false;
            }

            let scroll_area_response =
                scroll_area.show_rows(ui, img_size, self.total_rows, |ui, row_range| {
                    ui.spacing_mut().item_spacing = Vec2::new(0., 0.);

                    let preload_from = row_range.start.saturating_sub(self.config.preloaded_rows);

                    let preload_to = if row_range.end + self.config.preloaded_rows > self.total_rows
                    {
                        self.total_rows
                    } else {
                        row_range.end + self.config.preloaded_rows
                    };

                    //first we go over the visible ones
                    for r in row_range.start..row_range.end {
                        for i in r * self.images_per_row..(r + 1) * self.images_per_row {
                            self.load_unload_image(
                                i,
                                row_range.start,
                                row_range.end,
                                img_size,
                                image_store,
                            );
                        }
                    }

                    //then in the down direction as the user is most likely to scroll down
                    for r in row_range.end..self.total_rows {
                        for i in r * self.images_per_row..(r + 1) * self.images_per_row {
                            self.load_unload_image(
                                i,
                                preload_from,
                                preload_to,
                                img_size,
                                image_store,
                            );
                        }
                    }

                    //then up
                    for r in 0..row_range.start {
                        for i in r * self.images_per_row..(r + 1) * self.images_per_row {
                            self.load_unload_image(
                                i,
                                preload_from,
                                preload_to,
                                img_size,
                                image_store,
                            );
                        }
                    }

                    for r in row_range.clone() {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(0., 0.);
                            ui.add_space(remainder / 2.0);

                            for j in r * self.images_per_row..(r + 1) * self.images_per_row {
                                self.show_image_at(ui, ctx, j, img_size, image_store);
                            }
                        });
                    }

                    if !utils::are_inputs_muted(ctx)
                        && ui.input_mut(|i| i.consume_shortcut(&self.config.sc_scroll.kbd_shortcut))
                    {
                        ui.scroll_with_delta(Vec2::new(0., -(img_size * 0.5)));
                    }

                    self.prev_row_range_start = row_range.start;
                });

            self.prev_scroll_offset = scroll_area_response.state.offset.y;
            self.prev_img_size = img_size;
            self.prev_images_per_row = self.images_per_row;
        });
    }

    fn load_unload_image(
        &mut self,
        i: usize,
        preload_from: usize,
        preload_to: usize,
        image_size: f32,
        image_store: &mut ImageStore,
    ) {
        let img = &mut match self.imgs.get_mut(i) {
            Some(img) => img,
            None => return,
        };

        if i >= preload_from * self.images_per_row && i <= preload_to * self.images_per_row {
            //Double the square size so we have a little downscale going on
            //Looks better than without and won't impact speed much. Possibly add as a config
            if !img.registered {
                image_store.register_img(&img.path, Some((image_size * 2.) as u32));
                img.registered = true;
            }
        } else {
            image_store.deregister_img(&img.path);
            img.registered = false;
        }
    }

    fn show_image_at(
        &mut self,
        ui: &mut Ui,
        ctx: &egui::Context,
        index: usize,
        max_size: f32,
        image_store: &mut ImageStore,
    ) {
        let image = match self.imgs.get_mut(index) {
            Some(img) => img,
            None => return,
        };

        if let Some(resp) = image.ui(ui, [max_size, max_size], image_store) {
            if resp.clicked() {
                self.selected_image_name = Some(image.name.clone());
            }
            if resp.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            let return_callback = show_context_menu(&self.config.context_menu, &resp, &image.path);

            if let Some(return_callback) = return_callback {
                self.callback = Some(Callback::from_callback(
                    return_callback,
                    Some(image.path.clone()),
                ));

                tracing::info!("{:?}", self.callback);
            }
        }
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) {
        if utils::are_inputs_muted(ctx) {
            return;
        }

        if (ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_more_per_row.kbd_shortcut))
            || (ctx.input(|i| i.raw_scroll_delta.y) < 0. && ctx.input(|i| i.zoom_delta() != 1.)))
            && self.images_per_row <= 15
        {
            self.images_per_row += 1;
            self.set_total_rows();
        }

        if (ctx.input_mut(|i| i.consume_shortcut(&self.config.sc_less_per_row.kbd_shortcut))
            || (ctx.input(|i| i.raw_scroll_delta.y) > 0. && ctx.input(|i| i.zoom_delta() != 1.)))
            && self.images_per_row != 1
        {
            self.images_per_row -= 1;
            self.set_total_rows();
        }
    }

    pub fn selected_image_name(&mut self) -> Option<String> {
        //We want it to be consumed
        self.selected_image_name.take()
    }

    pub fn set_total_rows(&mut self) {
        //div_ceil will be available in the next release. Avoids conversions..
        self.total_rows = (self.imgs.len() as f32 / self.images_per_row as f32).ceil() as usize
    }

    pub fn pop(&mut self, path: &Path) {
        if let Some(pos) = self.imgs.iter().position(|x| x.path == path) {
            self.imgs.remove(pos);
            self.set_total_rows();
        }
    }

    pub fn take_callback(&mut self) -> Option<Callback> {
        self.callback.take()
    }

    pub fn reload_at(&mut self, path: &Path, image_store: &mut ImageStore) {
        if let Some(pos) = self.imgs.iter().position(|x| x.path == path)
            && let Some(img) = self.imgs.get_mut(pos)
        {
            image_store.reload(&img.path, Some((self.prev_img_size * 2.) as u32));
        }
    }
}

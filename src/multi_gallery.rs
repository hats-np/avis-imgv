use crate::{config::MultiGalleryConfig, thumbnail_image::ThumbnailImage};
use eframe::{
    egui::{self, Ui},
    epaint::Vec2,
};
use std::path::PathBuf;

pub struct MultiGallery {
    imgs: Vec<ThumbnailImage>,
    config: MultiGalleryConfig,
    selected_image_name: Option<String>,
}

impl MultiGallery {
    pub fn new(
        image_paths: &Vec<PathBuf>,
        config: MultiGalleryConfig,
        output_profile: &String,
    ) -> MultiGallery {
        let mut mg = MultiGallery {
            imgs: ThumbnailImage::from_paths(image_paths, output_profile),
            selected_image_name: None,
            config,
        };

        mg.imgs.sort_by(|a, b| a.name.cmp(&b.name));

        mg
    }

    pub fn ui(&mut self, ctx: &egui::Context, jump_to_index: &mut Option<usize>) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0., 0.);
            //div_ceil will be available in the next release. Avoids conversions..
            let total_rows =
                (self.imgs.len() as f32 / self.config.images_per_row as f32).ceil() as usize;
            let mut loading_imgs = self.imgs.iter().filter(|i| i.is_loading()).count();

            ui.set_min_width(ui.available_width());
            let img_size = ui.available_width() / self.config.images_per_row as f32;

            let mut scroll_area = egui::ScrollArea::vertical().drag_to_scroll(true);

            match jump_to_index.take() {
                Some(mut i) => {
                    //Get start of the row index so it's easier to calculate the offset
                    i = i - (i % self.config.images_per_row);
                    let scroll_offset = ((i as f32) / self.config.images_per_row as f32) * img_size;
                    scroll_area = scroll_area.scroll_offset(Vec2 {
                        x: 0.,
                        y: scroll_offset,
                    })
                }
                None => {}
            };

            scroll_area.show_rows(ui, img_size, total_rows, |ui, row_range| {
                ui.spacing_mut().item_spacing = Vec2::new(0., 0.);

                let preload_from = if row_range.start <= self.config.preloaded_rows {
                    0
                } else {
                    row_range.start - self.config.preloaded_rows
                };

                let preload_to = if row_range.end + self.config.preloaded_rows > total_rows {
                    total_rows
                } else {
                    row_range.end + self.config.preloaded_rows
                };

                //first we go over the visible ones
                for r in row_range.start..row_range.end {
                    for i in r * self.config.images_per_row..(r + 1) * self.config.images_per_row {
                        self.load_unload_image(
                            i,
                            row_range.start,
                            row_range.end,
                            &mut loading_imgs,
                        );
                    }
                }

                //then in the down direction as the user is most likely to scroll down
                for r in row_range.end..total_rows {
                    for i in r * self.config.images_per_row..(r + 1) * self.config.images_per_row {
                        self.load_unload_image(i, preload_from, preload_to, &mut loading_imgs);
                    }
                }

                //then up
                for r in 0..row_range.start {
                    for i in r * self.config.images_per_row..(r + 1) * self.config.images_per_row {
                        self.load_unload_image(i, preload_from, preload_to, &mut loading_imgs);
                    }
                }

                for r in row_range {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0., 0.);
                        for j in
                            r * self.config.images_per_row..(r + 1) * self.config.images_per_row
                        {
                            match &mut self.imgs.get_mut(j) {
                                Some(img) => Self::show_image(
                                    img,
                                    ui,
                                    img_size,
                                    &mut self.selected_image_name,
                                    &self.config.margin_size,
                                ),
                                None => {}
                            }
                        }
                    });
                }

                if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                    ui.scroll_with_delta(Vec2::new(0., (img_size * 0.5) * -1.));
                }
            });
        });
    }

    fn load_unload_image(
        &mut self,
        i: usize,
        preload_from: usize,
        preload_to: usize,
        loading_imgs: &mut usize,
    ) {
        let img = &mut match self.imgs.get_mut(i) {
            Some(img) => img,
            None => return,
        };

        if i >= preload_from * self.config.images_per_row
            && i <= preload_to * self.config.images_per_row
        {
            if loading_imgs != &self.config.simultaneous_load {
                if img.load(self.config.image_size) {
                    *loading_imgs += 1;
                }
            }
        } else {
            img.unload_delayed();
            img.unload(i);
        }
    }

    fn show_image(
        image: &mut ThumbnailImage,
        ui: &mut Ui,
        max_size: f32,
        select_image_name: &mut Option<String>,
        margin_size: &f32,
    ) {
        match image.ui(ui, [max_size, max_size], margin_size) {
            Some(resp) => {
                if resp.double_clicked() {
                    *select_image_name = Some(image.name.clone());
                }
            }
            None => {}
        };
    }

    pub fn selected_image_name(&mut self) -> Option<String> {
        //We want it to be consumed
        self.selected_image_name.take()
    }
}

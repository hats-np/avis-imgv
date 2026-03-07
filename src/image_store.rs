use crate::db::DbRepository;
use crate::image::Image;
use eframe::egui_wgpu::RenderState;
use epaint::{TextureId, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread::JoinHandle;

pub struct ImageStore {
    imgs: HashMap<PathBuf, StoredImage>,
    loading_imgs: HashMap<PathBuf, LoadingImage>,
    loading_queue: HashMap<PathBuf, QueuedImage>,
    output_icc_profile_name: String,
    max_texture_size: u32,
    error_img: Image, //TODO: Make it so error image texture is never freed
    load_budget_per_frame: usize,
    simultaneous_load: usize,
    db_repo: DbRepository,
    render_state: RenderState,
    raw_exiftool_preview_ext: Vec<String>,
}

struct StoredImage {
    image: Image,
    consumer_count: u32,
    desired_size: Option<u32>,
}

struct LoadingImage {
    image_handle: JoinHandle<Option<Image>>,
    consumer_count: u32,
    desired_size: Option<u32>,
}

struct QueuedImage {
    consumer_count: u32,
    desired_size: Option<u32>,
}

impl ImageStore {
    pub fn new(
        output_icc_profile_name: String,
        max_texture_size: u32,
        render_state: &RenderState,
        db_repo: &DbRepository,
        simultaneous_load: usize,
        raw_exiftool_preview_ext: &[String],
    ) -> ImageStore {
        let error_img = Image::get_error_image(render_state);
        ImageStore {
            imgs: HashMap::new(),
            loading_imgs: HashMap::new(),
            loading_queue: HashMap::new(),
            output_icc_profile_name,
            max_texture_size,
            load_budget_per_frame: 2, //Higher values can cause bad frametimes when loading a lot
            //of pictures at once
            error_img,
            simultaneous_load,
            db_repo: db_repo.clone(),
            render_state: render_state.clone(),
            raw_exiftool_preview_ext: raw_exiftool_preview_ext.to_vec(),
        }
    }
    pub fn is_image_loaded(&self, pathbuf: &PathBuf) -> bool {
        self.imgs.contains_key(pathbuf)
    }

    pub fn get_image_size(&self, pathbuf: &PathBuf) -> Option<Vec2> {
        self.imgs
            .get_key_value(pathbuf)
            .map(|stored_image| stored_image.1.image.size)
    }

    pub fn get_texture_id(&self, pathbuf: &PathBuf) -> Option<TextureId> {
        if let Some(stored_image) = self.imgs.get_key_value(pathbuf) {
            stored_image.1.image.get_texture_id()
        } else {
            None
        }
    }

    pub fn get_image_metadata(&self, pathbuf: &PathBuf) -> Option<&HashMap<String, String>> {
        if let Some(stored_image) = self.imgs.get_key_value(pathbuf) {
            Some(&stored_image.1.image.metadata)
        } else {
            None
        }
    }

    pub fn register_img(&mut self, pathbuf: &PathBuf, desired_size: Option<u32>) {
        let mut should_reload = false;
        let mut should_return = false;

        if let Some(img) = self.loading_queue.get_mut(pathbuf) {
            img.consumer_count += 1;
            should_return = true;
        } else if let Some(img) = self.loading_imgs.get_mut(pathbuf) {
            img.consumer_count += 1;
            should_return = true;
        } else if let Some(img) = self.imgs.get_mut(pathbuf) {
            img.consumer_count += 1;
            if img.desired_size.is_some() && desired_size.is_none() {
                should_reload = true;
            } else if let (Some(img_sz), Some(target_sz)) = (img.desired_size, desired_size)
                && img_sz < target_sz
            {
                should_reload = true;
            }
            should_return = true;
        }

        if should_reload {
            self.reload(pathbuf, desired_size);
        }

        if should_return {
            return;
        }

        self.loading_queue.insert(
            pathbuf.clone(),
            QueuedImage {
                consumer_count: 1,
                desired_size,
            },
        );
    }

    pub fn deregister_img(&mut self, pathbuf: &PathBuf) {
        if let Some(img) = self.imgs.get_mut(pathbuf) {
            img.consumer_count -= 1;
        } else if let Some(img) = self.loading_imgs.get_mut(pathbuf) {
            img.consumer_count -= 1;
        } else if let Some(img) = self.loading_queue.get_mut(pathbuf) {
            img.consumer_count -= 1;
        }
    }

    pub fn reload(&mut self, pathbuf: &PathBuf, desired_size: Option<u32>) {
        if let Some(img) = self.imgs.remove(pathbuf) {
            img.image.free_texture(&self.render_state);
            self.loading_queue.insert(
                pathbuf.clone(),
                QueuedImage {
                    consumer_count: img.consumer_count,
                    desired_size,
                },
            );
        }
    }

    pub fn dequeue_all_images_awaiting_load(&mut self) {
        let mut to_dequeue: Vec<PathBuf> = vec![];
        for (key, img) in &self.loading_queue {
            if img.consumer_count == 0 {
                continue;
            }

            if self.loading_imgs.len() >= self.simultaneous_load {
                break;
            }

            to_dequeue.push(key.clone());

            let image_handle = Image::load(
                key.clone(),
                img.desired_size,
                self.output_icc_profile_name.clone(),
                &self.render_state,
                self.max_texture_size,
                &self.db_repo,
                &self.raw_exiftool_preview_ext,
            );

            self.loading_imgs.insert(
                key.clone(),
                LoadingImage {
                    image_handle,
                    consumer_count: img.consumer_count,
                    desired_size: img.desired_size,
                },
            );
        }

        for key in to_dequeue {
            self.loading_queue.remove(&key);
        }
    }

    pub fn finish_loading_images(&mut self) {
        let mut imgs_to_finish_loading: Vec<PathBuf> = vec![];
        let mut imgs_to_drop: Vec<PathBuf> = vec![];
        for (key, img) in &self.loading_imgs {
            if !img.image_handle.is_finished() {
                continue;
            }

            if img.consumer_count == 0 {
                imgs_to_drop.push(key.clone());
                continue;
            }

            if imgs_to_finish_loading.len() < self.load_budget_per_frame {
                imgs_to_finish_loading.push(key.clone());
            }
        }

        for img_to_drop in &imgs_to_drop {
            self.loading_imgs.remove(img_to_drop);
        }

        for key in imgs_to_finish_loading {
            let loading_img = self.loading_imgs.remove(&key).unwrap();
            let img = if let Some(mut img) = loading_img.image_handle.join().unwrap() {
                img.register_texture(&self.render_state);
                img
            } else {
                self.error_img.clone() //cheap as only the texture_id is stored in the struct and
                //not the texture itself
            };

            self.imgs.insert(
                key,
                StoredImage {
                    image: img,
                    consumer_count: loading_img.consumer_count,
                    desired_size: loading_img.desired_size,
                },
            );
        }
    }

    pub fn unload_images_with_no_consumers(&mut self) {
        self.imgs.retain(|_path, img| {
            if img.consumer_count == 0 {
                //Avoid unloading our error image texture
                if let (Some(texture_id), Some(error_texture_id)) =
                    (img.image.get_texture_id(), self.error_img.get_texture_id())
                    && texture_id != error_texture_id
                {
                    img.image.free_texture(&self.render_state);
                }
                false
            } else {
                true
            }
        });
    }

    pub fn update(&mut self) {
        self.dequeue_all_images_awaiting_load();
        self.finish_loading_images();
        self.unload_images_with_no_consumers();
    }
}

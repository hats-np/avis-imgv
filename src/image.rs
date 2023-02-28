use eframe::{
    egui,
    epaint::{ColorImage, TextureHandle},
};
use image::DynamicImage;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::PathBuf,
    thread::{self, JoinHandle},
};

use crate::{
    icc::profile_desc_to_icc,
    metadata::{self, Orientation, METADATA_ORIENTATION, METADATA_PROFILE_DESCRIPTION},
};

pub struct Image {
    pub texture: Option<TextureHandle>,
    pub metadata: HashMap<String, String>,
    color_image: Option<ColorImage>,
}

impl Image {
    pub fn load(
        path: PathBuf,
        image_size: Option<u32>,
        output_icc_profile: String,
    ) -> JoinHandle<Option<Image>> {
        thread::spawn(move || {
            let mut f = match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    println!("{}", e);
                    return None;
                }
            };

            let mut buffer = Vec::new();
            match f.read_to_end(&mut buffer) {
                Ok(_) => {}
                Err(e) => {
                    println!("{}", e);
                    return None;
                }
            }

            let mut image = match image::load_from_memory(&buffer) {
                Ok(img) => img,
                Err(e) => {
                    println!("{}", e);
                    return None;
                }
            };

            if image_size.is_some() {
                image = Self::resize(image, image_size);
            }

            let metadata =
                metadata::Metadata::get_image_metadata(&path.to_string_lossy().to_string())
                    .unwrap_or_default();

            image = Self::orient(image, &metadata);

            let size = [image.width() as _, image.height() as _];
            let mut flat_samples = image.into_rgb8().into_flat_samples();
            let pixels = flat_samples.as_mut_slice();

            match metadata.get(METADATA_PROFILE_DESCRIPTION) {
                Some(cpd) => Self::apply_cc(&cpd, pixels, &path, &output_icc_profile),
                None => {}
            };

            return Some(Image {
                color_image: Some(ColorImage::from_rgb(size, pixels)),
                texture: None,
                metadata,
            });
        })
    }

    pub fn resize(img: DynamicImage, target_size: Option<u32>) -> DynamicImage {
        match target_size {
            Some(mut t_size) => {
                let largest_side = if img.width() > img.height() {
                    img.width()
                } else {
                    img.height()
                };

                let largest_side = largest_side as u32;
                t_size = if t_size > largest_side {
                    largest_side
                } else {
                    t_size
                };

                img.resize(t_size, t_size, image::imageops::FilterType::CatmullRom)
            }
            None => img,
        }
    }

    pub fn orient(img: DynamicImage, metadata: &HashMap<String, String>) -> DynamicImage {
        //see https://magnushoff.com/articles/jpeg-orientation/
        match metadata.get(METADATA_ORIENTATION) {
            Some(o) => match metadata::Orientation::from_orientation_metadata(&o) {
                Orientation::Normal => img,
                Orientation::MirrorHorizontal => img.fliph(),
                Orientation::Rotate180 => img.rotate180(),
                Orientation::MirrorVertical => img.flipv(),
                Orientation::MirrorHorizontalRotate270 => img.fliph().rotate270(),
                Orientation::Rotate90CW => img.rotate90(),
                Orientation::MirrorHorizontalRotate90CW => img.fliph().rotate90(),
                Orientation::Rotate270CW => img.rotate270(),
            },
            None => img,
        }
    }

    pub fn apply_cc(
        color_profile_desc: &str,
        mut pixels: &mut [u8],
        path: &PathBuf,
        output_profile: &String,
    ) {
        if color_profile_desc
            .to_lowercase()
            .contains(&output_profile.to_lowercase())
        {
            println!(
                "Input {} and output {} profiles are the same -> skipping",
                color_profile_desc, output_profile
            );
            return;
        }

        let input_icc_bytes = match profile_desc_to_icc(color_profile_desc) {
            Some(icc_bytes) => icc_bytes.to_vec(),
            None => {
                println!(
                    "No built in icc profile matching {} extracting from image",
                    color_profile_desc
                );
                match metadata::Metadata::extract_icc_from_image(&path) {
                    Some(icc_bytes) => {
                        println!("Successfully extract icc profile from image");
                        icc_bytes
                    }
                    None => return,
                }
            }
        };

        let output_icc_bytes = match profile_desc_to_icc(&output_profile) {
            Some(icc_bytes) => icc_bytes.to_vec(),
            None => {
                println!("Badly configured output icc profile -> {}", output_profile);
                return;
            }
        };

        let input_profile = match qcms::Profile::new_from_slice(&input_icc_bytes) {
            Some(profile) => profile,
            None => {
                println!("Failed constructing input qcms profile from icc data");
                return;
            }
        };

        let mut output_profile = match qcms::Profile::new_from_slice(&output_icc_bytes) {
            Some(profile) => profile,
            None => {
                println!("Failed constructing output qcms profile from icc data");
                return;
            }
        };

        output_profile.precache_output_transform();

        match qcms::Transform::new(
            &input_profile,
            &output_profile,
            qcms::DataType::RGB8,
            qcms::Intent::default(),
        ) {
            Some(transform) => transform.apply(&mut pixels),
            None => println!("Failure applying icc profile to image"),
        }
    }

    pub fn get_texture(&mut self, name: &str, ui: &mut egui::Ui) -> &Option<TextureHandle> {
        match &self.texture {
            Some(_) => &self.texture,
            None => match self.color_image.take() {
                Some(img) => {
                    self.texture = Some(ui.ctx().load_texture(name, img, Default::default()));
                    &self.texture
                }
                None => &None,
            },
        }
    }
}

use eframe::{
    egui,
    epaint::{ColorImage, TextureHandle},
};
use image::{DynamicImage, RgbImage};
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::PathBuf,
    thread::{self, JoinHandle},
};
use std::time::Instant;

use crate::{
    icc::profile_desc_to_icc,
    metadata::{self, Orientation, METADATA_ORIENTATION, METADATA_PROFILE_DESCRIPTION}
};

use fast_image_resize::{images::Image as FirImage, ResizeOptions};
use fast_image_resize::{PixelType, Resizer};

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
            let file_name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy();

            let mut now = Instant::now();
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
                    println!("{} -> Error reading image into buffer: {}", file_name, e);
                    return None;
                }
            }

            println!(
                "{} -> Spent {}ms reading into buffer",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            let mut image = Self::decode(&buffer, &file_name)?;

            println!(
                "{} -> Spent {}ms decoding",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            if image_size.is_some() {
                image = Self::resize(image, image_size);
            }

            println!(
                "{} -> Spent {}ms resizing",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            let metadata =
                metadata::Metadata::get_image_metadata(&path.to_string_lossy()).unwrap_or_default();

            println!(
                "{} -> Spent {}ms reading metadata",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            image = Self::orient(image, &metadata);

            println!(
                "{} -> Spent {}ms orienting",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            let size = [image.width() as _, image.height() as _];
            let mut flat_samples = image.into_rgb8().into_flat_samples();
            let pixels = flat_samples.as_mut_slice();

            if let Some(cpd) = metadata.get(METADATA_PROFILE_DESCRIPTION) {
                Self::apply_cc(cpd, pixels, &path, &output_icc_profile);
            };

            println!(
                "{} -> Spent {}ms applying CC",
                file_name,
                now.elapsed().as_millis()
            );

            Some(Image {
                color_image: Some(ColorImage::from_rgb(size, pixels)),
                texture: None,
                metadata,
            })
        })
    }

    pub fn decode(buffer: &[u8], file_name: &str) -> Option<DynamicImage> {
        match image::load_from_memory(buffer) {
            Ok(img) => Some(img),
            Err(e) => {
                println!("{} -> Failure decoding image: {}", file_name, e);
                None
            }
        }
    }

    pub fn resize(img: DynamicImage, target_size: Option<u32>) -> DynamicImage {
        match target_size {
            Some(target_size) => {
                let aspect_ratio = img.width() as f32 / img.height() as f32;
                let dest_width: u32;
                let dest_height: u32;
                if img.width() > img.height() {
                    dest_width = if img.width() > target_size {
                        target_size
                    } else {
                        img.width()
                    };
                    dest_height = (dest_width as f32 / aspect_ratio) as u32;
                } else {
                    dest_height = if img.height() > target_size {
                        target_size
                    } else {
                        img.width()
                    };
                    dest_width = (dest_height as f32 * aspect_ratio) as u32;
                };

                if dest_width == 0 || dest_height == 0 {
                    return img;
                }

                let src_image = match FirImage::from_vec_u8(
                    img.width(),
                    img.height(),
                    img.to_rgb8().into_raw(),
                    PixelType::U8x3,
                ) {
                    Ok(img) => img,
                    Err(e) => {
                        println!(
                            "Failure building fast_image_resize image from dynamic image -> {e}",
                        );
                        return img;
                    }
                };

                let mut dest_image =
                    FirImage::new(dest_width, dest_height, src_image.pixel_type());

                let mut resizer = Resizer::new();
                // By default, Resizer multiplies and divides by alpha channel
                // images with U8x2, U8x4, U16x2 and U16x4 pixels.
                match resizer.resize(&src_image, &mut dest_image, &ResizeOptions::new())
                {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Failure resizing image -> {e}");
                        return img;
                    }
                }

                match RgbImage::from_raw(
                    dest_width,
                    dest_height,
                    dest_image.buffer().to_vec(),
                ) {
                    Some(rgb_image) => DynamicImage::from(rgb_image),
                    None => {
                        println!("Failure building rgb image from resized image");
                        img
                    }
                }
            }
            None => img,
        }
    }

    pub fn orient(img: DynamicImage, metadata: &HashMap<String, String>) -> DynamicImage {
        //see https://magnushoff.com/articles/jpeg-orientation/
        match metadata.get(METADATA_ORIENTATION) {
            Some(o) => match metadata::Orientation::from_orientation_metadata(o) {
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

    ///Applies color conversion to the image
    pub fn apply_cc(
        color_profile_desc: &str,
        pixels: &mut [u8],
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
                match metadata::Metadata::extract_icc_from_image(path) {
                    Some(icc_bytes) => {
                        println!("Successfully extract icc profile from image");
                        icc_bytes
                    }
                    None => return,
                }
            }
        };

        let output_icc_bytes = match profile_desc_to_icc(output_profile) {
            Some(icc_bytes) => icc_bytes.to_vec(),
            None => {
                println!("Badly configured output icc profile -> {}", output_profile);
                return;
            }
        };

        let input_profile = match qcms::Profile::new_from_slice(&input_icc_bytes, false) {
            Some(profile) => profile,
            None => {
                println!("Failed constructing input qcms profile from icc data");
                return;
            }
        };

        let mut output_profile = match qcms::Profile::new_from_slice(&output_icc_bytes, false) {
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
            Some(transform) => transform.apply(pixels),
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

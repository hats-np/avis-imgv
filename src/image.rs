use crate::{
    db::Db,
    icc::profile_desc_to_icc,
    metadata::{self, Orientation, METADATA_ORIENTATION, METADATA_PROFILE_DESCRIPTION},
    JXL_EXTENSION, SKIP_ORIENT_EXTENSIONS,
};
use eframe::{
    egui,
    epaint::{ColorImage, TextureHandle},
};
use image::{DynamicImage, RgbImage};
use jpegxl_rs::decoder_builder;
use lcms2::*;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::PathBuf,
    thread::{self, JoinHandle},
};
use std::{path::Path, time::Instant};

use fast_image_resize::{images::Image as FirImage, ResizeOptions};
use fast_image_resize::{PixelType, Resizer};

pub const LOAD_FAIL_PNG: &[u8; 95764] = include_bytes!("../resources/load_fail.png");

pub struct Image {
    pub texture: TextureHandle,
    pub metadata: HashMap<String, String>,
}

impl Image {
    pub fn load(
        path: PathBuf,
        image_size: Option<u32>,
        output_icc_profile: String,
        ctx: &egui::Context,
    ) -> JoinHandle<Option<Image>> {
        let ctx = ctx.clone();
        thread::spawn(move || {
            let file_name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy();

            let mut now = Instant::now();
            let mut f = match File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    println!("Failure opening image: {e}");

                    let delete_result = Db::delete_file_by_path(&path);
                    if delete_result.is_err() {
                        println!("Failure deleting file record from the database {e}");
                    }

                    return get_error_image(&file_name, &ctx);
                }
            };

            let mut buffer = Vec::new();
            match f.read_to_end(&mut buffer) {
                Ok(_) => {}
                Err(e) => {
                    println!("{file_name} -> Error reading image into buffer: {e}");
                    return get_error_image(&file_name, &ctx);
                }
            }

            println!(
                "{} -> Spent {}ms reading into buffer",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            let mut image = Self::decode(&buffer, &file_name, &path)?;

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

            if !SKIP_ORIENT_EXTENSIONS.contains(
                &path
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default(),
            ) {
                image = Self::orient(image, &metadata);
            }

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
                texture: ctx.load_texture(
                    file_name,
                    ColorImage::from_rgb(size, pixels),
                    Default::default(),
                ),
                metadata,
            })
        })
    }

    pub fn decode(buffer: &[u8], file_name: &str, path: &Path) -> Option<DynamicImage> {
        if path.extension().unwrap_or_default() == JXL_EXTENSION {
            Self::decode_jxl(buffer, file_name)
        } else {
            Self::decode_generic(buffer, file_name)
        }
    }

    pub fn decode_jxl(buffer: &[u8], file_name: &str) -> Option<DynamicImage> {
        //JPEG XL has the option to execute with a parallel runner, but since we already manage
        //multithreading decoding by decoding one image per thread, it's better to decode each
        //individual image single threadedly.
        let decoder = match decoder_builder().build() {
            Ok(decoder) => decoder,
            Err(e) => {
                println!("Failure initiating JXL decoder for {file_name} -> {e}");
                return None;
            }
        };

        let r = decoder.decode_with::<u8>(buffer);

        match r {
            Ok((metadata, buf)) => match RgbImage::from_raw(metadata.width, metadata.height, buf) {
                Some(rgb_image) => Some(DynamicImage::from(rgb_image)),
                None => {
                    println!("Failure building rgb image from JXL decoded buffer for {file_name}");
                    None
                }
            },
            Err(e) => {
                println!("Failure creating rbimage from raw JXL buffer for {file_name} -> {e}");
                None
            }
        }
    }

    pub fn decode_generic(buffer: &[u8], file_name: &str) -> Option<DynamicImage> {
        match image::load_from_memory(buffer) {
            Ok(img) => Some(img),
            Err(e) => {
                println!("{file_name} -> Failure decoding image: {e}");
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

                let mut dest_image = FirImage::new(dest_width, dest_height, src_image.pixel_type());

                let mut resizer = Resizer::new();
                // By default, Resizer multiplies and divides by alpha channel
                // images with U8x2, U8x4, U16x2 and U16x4 pixels.
                match resizer.resize(&src_image, &mut dest_image, &ResizeOptions::new()) {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Failure resizing image -> {e}");
                        return img;
                    }
                }

                match RgbImage::from_raw(dest_width, dest_height, dest_image.buffer().to_vec()) {
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
                "Input {color_profile_desc} and output {output_profile} profiles are the same -> skipping"
            );
            return;
        }

        let input_icc_bytes = match profile_desc_to_icc(color_profile_desc) {
            Some(icc_bytes) => icc_bytes.to_vec(),
            None => {
                println!(
                    "No built-in ICC profile matching {color_profile_desc} extracting from image"
                );
                match metadata::Metadata::extract_icc_from_image(path) {
                    Some(icc_bytes) => {
                        println!("Successfully extracted ICC profile from image");
                        icc_bytes
                    }
                    None => return,
                }
            }
        };

        let output_icc_bytes = match profile_desc_to_icc(output_profile) {
            Some(icc_bytes) => icc_bytes.to_vec(),
            None => {
                println!("Badly configured output ICC profile -> {output_profile}");
                return;
            }
        };

        let input_profile = match Profile::new_icc(&input_icc_bytes) {
            Ok(profile) => profile,
            Err(_) => {
                println!("Failed constructing input lcms2 profile from ICC data");
                return;
            }
        };

        let output_profile = match Profile::new_icc(&output_icc_bytes) {
            Ok(profile) => profile,
            Err(_) => {
                println!("Failed constructing output lcms2 profile from ICC data");
                return;
            }
        };

        let transform = match Transform::new(
            &input_profile,
            PixelFormat::RGB_8,
            &output_profile,
            PixelFormat::RGB_8,
            Intent::Perceptual,
            //TransformFlags::NO_CACHE,
        ) {
            Ok(transform) => transform,
            Err(_) => {
                println!("Failure applying ICC profile to image");
                return;
            }
        };

        transform.transform_in_place(pixels);
    }
}

pub fn get_error_image(name: &str, ctx: &egui::Context) -> Option<Image> {
    let image = image::load_from_memory(LOAD_FAIL_PNG).unwrap();
    let size = [image.width() as _, image.height() as _];
    let mut flat_samples = image.into_rgb8().into_flat_samples();
    let pixels = flat_samples.as_mut_slice();

    Some(Image {
        texture: ctx.load_texture(name, ColorImage::from_rgb(size, pixels), Default::default()),
        metadata: HashMap::new(),
    })
}

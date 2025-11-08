use crate::{
    db::Db,
    icc::profile_desc_to_icc,
    metadata::{self, Orientation, METADATA_ORIENTATION, METADATA_PROFILE_DESCRIPTION},
    FRAME_MEMORY_KEY, JXL_EXTENSION, RAW_EXTENSIONS, SKIP_ORIENT_EXTENSIONS,
};
use eframe::{
    egui::{self, Id},
    egui_wgpu::RenderState,
    wgpu::{self},
};
use epaint::{TextureId, Vec2};
use image::{DynamicImage, RgbImage};
use jpegxl_rs::decoder_builder;
use lcms2::*;
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::PathBuf,
    process::Command,
    thread::{self, JoinHandle},
};
use std::{path::Path, time::Instant};

use fast_image_resize::{images::Image as FirImage, ResizeOptions};
use fast_image_resize::{PixelType, Resizer};

pub const LOAD_FAIL_PNG: &[u8; 95764] = include_bytes!("../resources/load_fail.png");

pub struct Image {
    pub texture_id: TextureId,
    pub size: Vec2,
    pub render_state: Option<RenderState>,
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

            let mut buffer = Vec::new();
            if RAW_EXTENSIONS.contains(
                &path
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .to_lowercase()
                    .as_str(),
            ) {
                match extract_preview_from_raw_file(&path) {
                    Some(buf) => buffer = buf,
                    None => return Self::get_error_image(&file_name, &ctx),
                };
            } else {
                let mut f = match File::open(&path) {
                    Ok(f) => f,
                    Err(e) => {
                        tracing::error!("Failure opening image: {e}");

                        let delete_result = Db::delete_file_by_path(&path);
                        if delete_result.is_err() {
                            tracing::error!("Failure deleting file record from the database {e}");
                        }

                        return Self::get_error_image(&file_name, &ctx);
                    }
                };

                match f.read_to_end(&mut buffer) {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("{file_name} -> Error reading image into buffer: {e}");
                        return Self::get_error_image(&file_name, &ctx);
                    }
                }
            }

            tracing::info!(
                "{} -> Spent {}ms reading into buffer",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            let mut image = match Self::decode(&buffer, &file_name, &path) {
                Some(img) => img,
                None => {
                    return Self::get_error_image(&file_name, &ctx);
                }
            };

            tracing::info!(
                "{} -> Spent {}ms decoding",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            if image_size.is_some() {
                image = Self::resize(image, image_size);
            }

            tracing::info!(
                "{} -> Spent {}ms resizing",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            let metadata =
                metadata::Metadata::get_image_metadata(&path.to_string_lossy()).unwrap_or_default();

            tracing::info!(
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

            tracing::info!(
                "{} -> Spent {}ms orienting",
                file_name,
                now.elapsed().as_millis()
            );
            now = Instant::now();

            let size: [u32; 2] = [image.width() as _, image.height() as _];
            let mut flat_samples = image.into_rgb8().into_flat_samples();
            let pixels = flat_samples.as_mut_slice();

            if let Some(cpd) = metadata.get(METADATA_PROFILE_DESCRIPTION) {
                Self::apply_cc(cpd, pixels, &path, &output_icc_profile);
            };

            tracing::info!(
                "{} -> Spent {}ms applying CC",
                file_name,
                now.elapsed().as_millis()
            );

            match Self::load_wgpu_linear_texture(pixels, size, &ctx, &file_name) {
                Some((texture_id, render_state)) => {
                    tracing::info!(
                        "Spent {}ms loading texture with wgpu",
                        now.elapsed().as_millis()
                    );
                    Some(Image {
                        texture_id,
                        size: Vec2 {
                            x: size[0] as f32,
                            y: size[1] as f32,
                        },
                        metadata,
                        render_state: Some(render_state),
                    })
                }
                None => Self::get_error_image(&file_name, &ctx),
            }
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
                tracing::error!("Failure initiating JXL decoder for {file_name} -> {e}");
                return None;
            }
        };

        let r = decoder.decode_with::<u8>(buffer);

        match r {
            Ok((metadata, buf)) => match RgbImage::from_raw(metadata.width, metadata.height, buf) {
                Some(rgb_image) => Some(DynamicImage::from(rgb_image)),
                None => {
                    tracing::error!(
                        "Failure building rgb image from JXL decoded buffer for {file_name}"
                    );
                    None
                }
            },
            Err(e) => {
                tracing::error!(
                    "Failure creating rbimage from raw JXL buffer for {file_name} -> {e}"
                );
                None
            }
        }
    }

    pub fn decode_generic(buffer: &[u8], file_name: &str) -> Option<DynamicImage> {
        match image::load_from_memory(buffer) {
            Ok(img) => Some(img),
            Err(e) => {
                tracing::info!("{file_name} -> Failure decoding image: {e}");
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
                        tracing::error!(
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
                        tracing::error!("Failure resizing image -> {e}");
                        return img;
                    }
                }

                match RgbImage::from_raw(dest_width, dest_height, dest_image.buffer().to_vec()) {
                    Some(rgb_image) => DynamicImage::from(rgb_image),
                    None => {
                        tracing::error!("Failure building rgb image from resized image");
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
            tracing::info!(
                "Input {color_profile_desc} and output {output_profile} profiles are the same -> skipping"
            );
            return;
        }

        let input_icc_bytes = match profile_desc_to_icc(color_profile_desc) {
            Some(icc_bytes) => icc_bytes.to_vec(),
            None => {
                tracing::info!(
                    "No built-in ICC profile matching {color_profile_desc} extracting from image"
                );
                match metadata::Metadata::extract_icc_from_image(path) {
                    Some(icc_bytes) => {
                        tracing::info!("Successfully extracted ICC profile from image");
                        icc_bytes
                    }
                    None => return,
                }
            }
        };

        let output_icc_bytes = match profile_desc_to_icc(output_profile) {
            Some(icc_bytes) => icc_bytes.to_vec(),
            None => {
                tracing::error!("Badly configured output ICC profile -> {output_profile}");
                return;
            }
        };

        let input_profile = match Profile::new_icc(&input_icc_bytes) {
            Ok(profile) => profile,
            Err(_) => {
                tracing::error!("Failed constructing input lcms2 profile from ICC data");
                return;
            }
        };

        let output_profile = match Profile::new_icc(&output_icc_bytes) {
            Ok(profile) => profile,
            Err(_) => {
                tracing::error!("Failed constructing output lcms2 profile from ICC data");
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
                tracing::error!("Failure applying ICC profile to image");
                return;
            }
        };

        transform.transform_in_place(pixels);
    }

    pub fn load_wgpu_linear_texture(
        pixels: &[u8],
        size: [u32; 2],
        ctx: &egui::Context,
        file_name: &str,
    ) -> Option<(TextureId, RenderState)> {
        let rgba_pixels: Vec<u8> = pixels
            .chunks_exact(3)
            .flat_map(|rgb| [rgb[0], rgb[1], rgb[2], 255])
            .collect();

        let texture_size = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };

        let render_state =
            match ctx.memory(|x| x.data.get_temp::<RenderState>(Id::new(FRAME_MEMORY_KEY))) {
                Some(rs) => rs,
                None => {
                    tracing::error!(
                        "Failure fetching render state from context, returning error image"
                    );
                    return None;
                }
            };

        let texture = render_state
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(&file_name),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // *** CRUCIAL: LINEAR format ***
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

        render_state.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size[0] * 4),
                rows_per_image: Some(size[1]),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let egui_texture_id = {
            let re = render_state.clone();
            let mut renderer = re.renderer.write();
            renderer.register_native_texture(
                &render_state.device,
                &texture_view,
                wgpu::FilterMode::Linear,
            )
        };

        Some((egui_texture_id, render_state))
    }

    pub fn get_error_image(file_name: &str, ctx: &egui::Context) -> Option<Image> {
        let image = image::load_from_memory(LOAD_FAIL_PNG).unwrap();
        let size = [image.width() as _, image.height() as _];
        let mut flat_samples = image.into_rgb8().into_flat_samples();
        let pixels = flat_samples.as_mut_slice();

        match Self::load_wgpu_linear_texture(pixels, size, &ctx, &file_name) {
            Some((texture_id, render_state)) => Some(Image {
                texture_id,
                size: Vec2 {
                    x: size[0] as f32,
                    y: size[1] as f32,
                },
                metadata: HashMap::new(),
                render_state: Some(render_state),
            }),
            None => None,
        }
    }
}

pub fn extract_preview_from_raw_file(path: &Path) -> Option<Vec<u8>> {
    let mut command = Command::new("exiftool");
    command.arg("-b").arg("-PreviewImage").arg(path);

    let output = match command.output() {
        Ok(output) => output,
        Err(e) => {
            tracing::error!("Failure fetching raw image preview with exiftool: {e}");
            return None;
        }
    };

    if output.status.success() {
        let std_out = output.stdout;

        if std_out.is_empty() {
            tracing::error!(
                "Extracted an empty image, raw likely does not have embeded preview jpg"
            );
            return None;
        }

        Some(std_out)
    } else {
        let error_message = String::from_utf8_lossy(&output.stderr);
        tracing::error!("Failure fetching raw image preview with exiftool: {error_message}");
        None
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if let Some(rs) = self.render_state.clone() {
            rs.renderer.write().free_texture(&self.texture_id);
        }
    }
}

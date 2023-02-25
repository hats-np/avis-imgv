use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, vec};

use crate::{APPLICATION, ORGANIZATION, QUALIFIER};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub gallery: GalleryConfig,
    pub multi_gallery: MultiGalleryConfig,
    #[serde(default = "default_limit_cached")]
    pub limit_cached: u32,
    #[serde(default = "default_output_icc_profile")]
    pub output_icc_profile: String,
    #[serde(default = "default_text_scaling")]
    pub text_scaling: f32,
}

#[derive(Deserialize, Serialize)]
pub struct GalleryConfig {
    #[serde(default = "default_nr_loaded_images")]
    pub nr_loaded_images: usize,
    #[serde(default = "default_should_wait")]
    pub should_wait: bool,
    #[serde(default = "default_metadata_tags")]
    pub metadata_tags: Vec<String>,
    #[serde(default = "default_frame_size_relative_to_image")]
    pub frame_size_relative_to_image: f32,
}

#[derive(Deserialize, Serialize)]
pub struct MultiGalleryConfig {
    #[serde(default = "default_images_per_row")]
    pub images_per_row: usize,
    #[serde(default = "default_preloaded_rows")]
    pub preloaded_rows: usize,
    #[serde(default = "default_image_size")]
    pub image_size: u32,
    #[serde(default = "default_simultaneous_load")]
    pub simultaneous_load: usize,
    #[serde(default = "default_margin_size")]
    pub margin_size: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            gallery: GalleryConfig::default(),
            multi_gallery: MultiGalleryConfig::default(),
            limit_cached: default_limit_cached(),
            output_icc_profile: default_output_icc_profile(),
            text_scaling: default_text_scaling(),
        }
    }
}

impl Default for GalleryConfig {
    fn default() -> Self {
        GalleryConfig {
            nr_loaded_images: default_nr_loaded_images(),
            should_wait: default_should_wait(),
            metadata_tags: default_metadata_tags(),
            frame_size_relative_to_image: default_frame_size_relative_to_image(),
        }
    }
}

impl Default for MultiGalleryConfig {
    fn default() -> Self {
        MultiGalleryConfig {
            images_per_row: default_images_per_row(),
            preloaded_rows: default_preloaded_rows(),
            image_size: default_image_size(),
            simultaneous_load: default_simultaneous_load(),
            margin_size: default_margin_size(),
        }
    }
}

impl Config {
    pub fn new() -> Config {
        let config_dir = match directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        {
            Some(dirs) => dirs.config_dir().to_owned(),
            None => return Config::default(),
        };

        let cfg_path = config_dir.join(PathBuf::from("config.yaml"));
        println!("Reading config -> {}", cfg_path.display());

        let config_yaml = match fs::read_to_string(cfg_path) {
            Ok(yaml) => yaml,
            Err(e) => {
                println!("Failure reading config file -> {}", e);
                return Config::default();
            }
        };

        let cfg = match serde_yaml::from_str(&config_yaml) {
            Ok(cfg) => cfg,
            Err(_) => {
                println!("Failure parsing config yaml, using defaults");
                Config::default()
            }
        };

        println!("Using config:");
        println!("{}", serde_yaml::to_string(&cfg).unwrap());

        cfg
    }
}

pub fn default_limit_cached() -> u32 {
    100000
}

pub fn default_output_icc_profile() -> String {
    String::from("srgb")
}

pub fn default_text_scaling() -> f32 {
    1.25
}

//Gallery
pub fn default_nr_loaded_images() -> usize {
    5
}
pub fn default_should_wait() -> bool {
    true
}
pub fn default_metadata_tags() -> Vec<String> {
    return vec![
        "Date/Time Original".to_string(),
        "Created Date".to_string(),
        "Camera Model Name".to_string(),
        "Lens Model".to_string(),
        "Focal Length".to_string(),
        "Aperture Value".to_string(),
        "Exposure Time".to_string(),
        "ISO".to_string(),
        "Image Size".to_string(),
        "Color Space".to_string(),
        "Directory".to_string(),
    ];
}
pub fn default_frame_size_relative_to_image() -> f32 {
    0.2
}

//Multi Gallery
pub fn default_images_per_row() -> usize {
    3
}
pub fn default_preloaded_rows() -> usize {
    2
}
pub fn default_image_size() -> u32 {
    1000
}
pub fn default_simultaneous_load() -> usize {
    8
}
pub fn default_margin_size() -> f32 {
    10.
}

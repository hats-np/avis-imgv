pub mod app;
pub mod callback;
pub mod config;
pub mod crawler;
pub mod db;
pub mod gallery_image;
pub mod icc;
pub mod image;
pub mod metadata;
pub mod multi_gallery;
pub mod navigator;
pub mod single_gallery;
pub mod theme;
pub mod thumbnail_image;
pub mod tree;
pub mod user_action;
pub mod utils;
pub mod perf_metrics;

pub const QUALIFIER: &str = "com";
pub const ORGANIZATION: &str = "avis-imgv";
pub const APPLICATION: &str = "avis-imgv";

pub const VALID_EXTENSIONS: &[&str] = &["jpg", "png", "jpeg", "webp", "gif", "bmp", "tiff"];

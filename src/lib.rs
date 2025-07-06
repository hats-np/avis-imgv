extern crate core;

use eframe::egui;

pub mod app;
pub mod callback;
pub mod config;
pub mod crawler;
pub mod db;
pub mod dropdown;
pub mod filters;
pub mod gallery_image;
pub mod grid_view;
pub mod icc;
pub mod image;
pub mod image_view;
pub mod metadata;
pub mod navigator;
pub mod perf_metrics;
pub mod theme;
pub mod thumbnail_image;
pub mod tree;
pub mod user_action;
pub mod utils;

pub const QUALIFIER: &str = "com";
pub const ORGANIZATION: &str = "avis-imgv";
pub const APPLICATION: &str = "avis-imgv";

pub const VALID_EXTENSIONS: &[&str] = &["jpg", "png", "jpeg", "webp", "gif", "bmp", "tiff"];
pub const ZUNE_JPEG_TYPES: &[&str] = &["jpg", "jpeg"];

pub fn no_icon(
    _ui: &egui::Ui,
    _rect: egui::Rect,
    _visuals: &egui::style::WidgetVisuals,
    _is_open: bool,
    _above_or_below: egui::AboveOrBelow,
) {
}

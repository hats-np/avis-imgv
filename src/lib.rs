use core::fmt;

use eframe::egui;

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
pub mod perf_metrics;
pub mod single_gallery;
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

#[derive(PartialEq, Debug, Clone)]
pub enum Order {
    Asc,
    Desc,
    DateAsc,
    DateDesc,
    Random,
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Order::Asc => write!(f, "Asc"),
            Order::Desc => write!(f, "Desc"),
            Order::DateDesc => write!(f, "Date Desc"),
            Order::DateAsc => write!(f, "Date Asc"),
            Order::Random => write!(f, "Random"),
        }
    }
}

pub fn no_icon(
    _ui: &egui::Ui,
    _rect: egui::Rect,
    _visuals: &egui::style::WidgetVisuals,
    _is_open: bool,
    _above_or_below: egui::AboveOrBelow,
) {
}

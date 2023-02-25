pub mod app;
pub mod config;
pub mod crawler;
pub mod db;
pub mod gallery_image;
pub mod icc;
pub mod image;
pub mod metadata;
pub mod multi_gallery;
pub mod single_gallery;
pub mod theme;
pub mod thumbnail_image;

pub const QUALIFIER: &'static str = "com";
pub const ORGANIZATION: &'static str = "avis-imgv";
pub const APPLICATION: &'static str = "avis-imgv";

pub const VALID_EXTENSIONS: &'static [&'static str] =
    &["jpg", "png", "jpeg", "webp", "gif", "bmp", "tiff"];
pub const METADATA_PROFILE_DESCRIPTION: &'static str = "Profile Description";
pub const METADATA_ORIENTATION: &'static str = "Orientation";

use crate::{callback::Callback, utils, APPLICATION, ORGANIZATION, QUALIFIER};
use eframe::egui::{Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, vec};

const MOD_ALT: &str = "alt";
const MOD_SHIFT: &str = "shift";
const MOD_CTRL: &str = "ctrl";
const MOD_MAC_CMD: &str = "mac_cmd";
const MOD_CMD: &str = "cmd";

#[derive(Deserialize, Serialize, Default)]
pub struct Config {
    pub image_view: ImageViewConfig,
    pub grid_view: GridViewConfig,
    pub general: GeneralConfig,
    pub filter: FilterConfig,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct GeneralConfig {
    #[serde(default = "default_limit_cached")]
    pub limit_cached: u32,
    #[serde(default = "default_output_icc_profile")]
    pub output_icc_profile: String,
    #[serde(default = "default_text_scaling")]
    pub text_scaling: f32,
    #[serde(default = "default_metadata_tags")]
    pub metadata_tags: Vec<String>,

    #[serde(default = "default_sc_toggle_gallery")]
    pub sc_toggle_gallery: Shortcut,
    #[serde(default = "default_sc_exit")]
    pub sc_exit: Shortcut,
    #[serde(default = "default_sc_menu")]
    pub sc_menu: Shortcut,
    #[serde(default = "default_sc_navigator")]
    pub sc_navigator: Shortcut,
    #[serde(default = "default_sc_dir_tree")]
    pub sc_dir_tree: Shortcut,
    #[serde(default = "default_sc_flatten_dir")]
    pub sc_flatten_dir: Shortcut,
    #[serde(default = "default_sc_watch_directory")]
    pub sc_watch_directory: Shortcut,
    #[serde(default = "default_sc_toggle_side_panel")]
    pub sc_toggle_side_panel: Shortcut,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ImageViewConfig {
    #[serde(default = "default_nr_loaded_images")]
    pub nr_loaded_images: usize,
    #[serde(default = "default_should_wait")]
    pub should_wait: bool,
    #[serde(default = "default_frame_size_relative_to_image")]
    pub frame_size_relative_to_image: f32,
    #[serde(default = "default_scroll_navigation")]
    pub scroll_navigation: bool,
    #[serde(default = "default_name_format")]
    pub name_format: String,
    #[serde(default = "default_user_actions")]
    pub user_actions: Vec<UserAction>,
    #[serde(default = "default_ctx_menu")]
    pub context_menu: Vec<ContextMenuEntry>,

    #[serde(default = "default_sc_fit")]
    pub sc_fit: Shortcut,
    #[serde(default = "default_sc_frame")]
    pub sc_frame: Shortcut,
    #[serde(default = "default_sc_zoom")]
    pub sc_zoom: Shortcut,
    #[serde(default = "default_sc_next")]
    pub sc_next: Shortcut,
    #[serde(default = "default_sc_prev")]
    pub sc_prev: Shortcut,
    #[serde(default = "default_sc_one_to_one")]
    pub sc_one_to_one: Shortcut,
    #[serde(default = "default_sc_fit_horizontal")]
    pub sc_fit_horizontal: Shortcut,
    #[serde(default = "default_sc_fit_vertical")]
    pub sc_fit_vertical: Shortcut,
    #[serde(default = "default_sc_fit_maximize")]
    pub sc_fit_maximize: Shortcut,
    #[serde(default = "default_sc_latch_fit_maximize")]
    pub sc_latch_fit_maximize: Shortcut,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct GridViewConfig {
    #[serde(default = "default_images_per_row")]
    pub images_per_row: usize,
    #[serde(default = "default_preloaded_rows")]
    pub preloaded_rows: usize,
    #[serde(default = "default_simultaneous_load")]
    pub simultaneous_load: usize,
    #[serde(default = "default_ctx_menu")]
    pub context_menu: Vec<ContextMenuEntry>,

    #[serde(default = "default_sc_scroll")]
    pub sc_scroll: Shortcut,
    #[serde(default = "default_sc_more_per_row")]
    pub sc_more_per_row: Shortcut,
    #[serde(default = "default_sc_less_per_row")]
    pub sc_less_per_row: Shortcut,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct FilterConfig {
    #[serde(default = "default_exif_tags")]
    pub exif_tags: Vec<FilterableExifTag>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct FilterableExifTag {
    pub name: String,
    pub fetch_distinct: bool,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(from = "ShortcutData")]
pub struct Shortcut {
    pub key: String,
    pub modifiers: Vec<String>,
    #[serde(skip)]
    #[serde(default = "default_shortcut")]
    pub kbd_shortcut: KeyboardShortcut,
}

#[derive(Deserialize, Serialize)]
pub struct ShortcutData {
    pub key: String,
    pub modifiers: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct UserAction {
    pub shortcut: Shortcut,
    pub exec: String,
    pub callback: Option<Callback>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct ContextMenuEntry {
    pub description: String,
    pub exec: String,
    pub callback: Option<Callback>,
}

impl Shortcut {
    fn from(key: &str, modifiers: &[&str]) -> Shortcut {
        let modifiers: Vec<String> = modifiers.iter().map(|x| x.to_string()).collect();
        Shortcut {
            kbd_shortcut: build_keyboard_shortcut(&modifiers, key),
            key: key.to_string(),
            modifiers,
        }
    }
}

impl Default for GeneralConfig {
    fn default() -> Self {
        GeneralConfig {
            limit_cached: default_limit_cached(),
            output_icc_profile: default_output_icc_profile(),
            text_scaling: default_text_scaling(),
            metadata_tags: default_metadata_tags(),
            sc_toggle_gallery: default_sc_toggle_gallery(),
            sc_toggle_side_panel: default_sc_toggle_side_panel(),
            sc_exit: default_sc_exit(),
            sc_menu: default_sc_menu(),
            sc_navigator: default_sc_navigator(),
            sc_dir_tree: default_sc_dir_tree(),
            sc_flatten_dir: default_sc_flatten_dir(),
            sc_watch_directory: default_sc_watch_directory(),
        }
    }
}

impl Default for ImageViewConfig {
    fn default() -> Self {
        ImageViewConfig {
            nr_loaded_images: default_nr_loaded_images(),
            should_wait: default_should_wait(),
            frame_size_relative_to_image: default_frame_size_relative_to_image(),
            scroll_navigation: default_scroll_navigation(),
            user_actions: default_user_actions(),
            context_menu: default_ctx_menu(),
            name_format: default_name_format(),

            sc_fit: default_sc_fit(),
            sc_frame: default_sc_frame(),
            sc_zoom: default_sc_zoom(),
            sc_next: default_sc_next(),
            sc_prev: default_sc_prev(),
            sc_one_to_one: default_sc_one_to_one(),
            sc_fit_vertical: default_sc_fit_vertical(),
            sc_fit_horizontal: default_sc_fit_horizontal(),
            sc_fit_maximize: default_sc_fit_maximize(),
            sc_latch_fit_maximize: default_sc_latch_fit_maximize(),
        }
    }
}

impl Default for GridViewConfig {
    fn default() -> Self {
        GridViewConfig {
            images_per_row: default_images_per_row(),
            preloaded_rows: default_preloaded_rows(),
            simultaneous_load: default_simultaneous_load(),
            context_menu: default_ctx_menu(),

            sc_scroll: default_sc_scroll(),
            sc_more_per_row: default_sc_more_per_row(),
            sc_less_per_row: default_sc_less_per_row(),
        }
    }
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig {
            exif_tags: default_exif_tags(),
        }
    }
}

impl Config {
    pub fn new() -> Config {
        Self::fetch_cfg()
    }

    pub fn fetch_cfg() -> Config {
        let config_dir = match directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
        {
            Some(dirs) => dirs.config_dir().to_owned(),
            None => return Config::default(),
        };

        let cfg_path = config_dir.join(PathBuf::from("config.json"));
        println!("Reading config -> {}", cfg_path.display());

        let config_json = match fs::read_to_string(cfg_path) {
            Ok(json) => json,
            Err(e) => {
                println!("Failure reading config file -> {e}");
                return Config::default();
            }
        };

        let cfg = match serde_json::from_str(&config_json) {
            Ok(cfg) => cfg,
            Err(e) => {
                println!("{e}");
                println!("Failure parsing config json, using defaults");
                Config::default()
            }
        };

        println!("Using config:");
        println!("{}", serde_json::to_string(&cfg).unwrap());

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

pub fn default_sc_toggle_gallery() -> Shortcut {
    Shortcut::from("Backspace", &[])
}

pub fn default_sc_exit() -> Shortcut {
    Shortcut::from("q", &[])
}

pub fn default_sc_menu() -> Shortcut {
    Shortcut::from("F1", &[])
}

pub fn default_sc_navigator() -> Shortcut {
    Shortcut::from("l", &[MOD_CTRL])
}

pub fn default_sc_dir_tree() -> Shortcut {
    Shortcut::from("t", &[])
}

pub fn default_sc_flatten_dir() -> Shortcut {
    Shortcut::from("f", &[MOD_CTRL])
}

pub fn default_sc_watch_directory() -> Shortcut {
    Shortcut::from("w", &[MOD_CTRL])
}

//Gallery
pub fn default_nr_loaded_images() -> usize {
    4
}
pub fn default_should_wait() -> bool {
    true
}
pub fn default_metadata_tags() -> Vec<String> {
    vec![
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
    ]
}
pub fn default_frame_size_relative_to_image() -> f32 {
    0.2
}
pub fn default_scroll_navigation() -> bool {
    true
}
pub fn default_name_format() -> String {
    "$(#File Name#)$( • ƒ#Aperture#)$( • #Shutter Speed#)$( • #ISO# ISO)".to_string()
}
pub fn default_user_actions() -> Vec<UserAction> {
    vec![]
}
pub fn default_ctx_menu() -> Vec<ContextMenuEntry> {
    vec![]
}
pub fn default_sc_fit() -> Shortcut {
    Shortcut::from("f", &[])
}
pub fn default_sc_frame() -> Shortcut {
    Shortcut::from("g", &[])
}
pub fn default_sc_toggle_side_panel() -> Shortcut {
    Shortcut::from("i", &[])
}
pub fn default_sc_zoom() -> Shortcut {
    Shortcut::from("Space", &[])
}
pub fn default_sc_next() -> Shortcut {
    Shortcut::from("ArrowRight", &[])
}
pub fn default_sc_prev() -> Shortcut {
    Shortcut::from("ArrowLeft", &[])
}
pub fn default_sc_one_to_one() -> Shortcut {
    Shortcut::from("1", &[MOD_ALT])
}
pub fn default_sc_fit_vertical() -> Shortcut {
    Shortcut::from("v", &[])
}
pub fn default_sc_fit_horizontal() -> Shortcut {
    Shortcut::from("h", &[])
}
pub fn default_sc_fit_maximize() -> Shortcut {
    Shortcut::from("m", &[])
}
pub fn default_sc_latch_fit_maximize() -> Shortcut {
    Shortcut::from("m", &[MOD_CTRL])
}

//Multi Gallery
pub fn default_images_per_row() -> usize {
    5
}
pub fn default_preloaded_rows() -> usize {
    1
}
pub fn default_simultaneous_load() -> usize {
    8
}
pub fn default_sc_scroll() -> Shortcut {
    Shortcut::from("Space", &[])
}
pub fn default_sc_more_per_row() -> Shortcut {
    Shortcut::from("Plus", &[])
}
pub fn default_sc_less_per_row() -> Shortcut {
    Shortcut::from("Minus", &[])
}

//Filter
pub fn default_exif_tags() -> Vec<FilterableExifTag> {
    vec![]
}

//Shortcuts
pub fn default_shortcut() -> KeyboardShortcut {
    //Bogus shortcut as default so we don't have to use option
    //Easier when implementing the shortcuts
    //We use F20 as most users don't have it and all modifiers
    let modi = Modifiers {
        alt: true,
        ctrl: true,
        shift: true,
        command: true,
        mac_cmd: false,
    };

    KeyboardShortcut::new(modi, Key::F20)
}

impl From<ShortcutData> for Shortcut {
    fn from(data: ShortcutData) -> Self {
        Shortcut {
            kbd_shortcut: build_keyboard_shortcut(&data.modifiers, &data.key),
            key: data.key,
            modifiers: data.modifiers,
        }
    }
}

pub fn build_keyboard_shortcut(mods: &[String], key: &str) -> KeyboardShortcut {
    let mut modifiers = Modifiers::default();
    for modi in mods {
        match modi.as_str() {
            MOD_ALT => modifiers.alt = true,
            MOD_CTRL => modifiers.ctrl = true,
            MOD_SHIFT => modifiers.shift = true,
            MOD_CMD => modifiers.command = true,
            MOD_MAC_CMD => modifiers.mac_cmd = true,
            _ => {
                println!("Invalid modifier({}) in configuration", modi.as_str())
            }
        }
    }

    match Key::from_name(&utils::capitalize_first_char(key)) {
        Some(key) => KeyboardShortcut {
            logical_key: key,
            modifiers,
        },
        None => {
            println!("Invalid shortcut key: {key}");
            default_shortcut()
        } //uses default unreachable shortcut
    }
}

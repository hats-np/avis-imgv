use eframe::egui::{Key, KeyboardShortcut, Modifiers};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::PathBuf, vec};

use crate::{callback::Callback, APPLICATION, ORGANIZATION, QUALIFIER};

const MOD_ALT: &str = "alt";
const MOD_SHIFT: &str = "shift";
const MOD_CTRL: &str = "ctrl";
const MOD_MAC_CMD: &str = "mac_cmd";
const MOD_CMD: &str = "cmd";

#[derive(Deserialize, Serialize, Default)]
pub struct Config {
    pub gallery: GalleryConfig,
    pub multi_gallery: MultiGalleryConfig,
    pub general: GeneralConfig,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct GeneralConfig {
    #[serde(default = "default_limit_cached")]
    pub limit_cached: u32,
    #[serde(default = "default_output_icc_profile")]
    pub output_icc_profile: String,
    #[serde(default = "default_text_scaling")]
    pub text_scaling: f32,

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
}

#[derive(Deserialize, Serialize, Clone)]
pub struct GalleryConfig {
    #[serde(default = "default_nr_loaded_images")]
    pub nr_loaded_images: usize,
    #[serde(default = "default_should_wait")]
    pub should_wait: bool,
    #[serde(default = "default_metadata_tags")]
    pub metadata_tags: Vec<String>,
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
    #[serde(default = "default_sc_metadata")]
    pub sc_metadata: Shortcut,
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
}

#[derive(Deserialize, Serialize, Clone)]
pub struct MultiGalleryConfig {
    #[serde(default = "default_images_per_row")]
    pub images_per_row: usize,
    #[serde(default = "default_preloaded_rows")]
    pub preloaded_rows: usize,
    #[serde(default = "default_simultaneous_load")]
    pub simultaneous_load: usize,
    #[serde(default = "default_margin_size")]
    pub margin_size: f32,
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
pub struct Shortcut {
    pub key: String,
    pub modifiers: Vec<String>,
    #[serde(skip)]
    #[serde(default = "default_shortcut")]
    pub kbd_shortcut: KeyboardShortcut,
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
        Shortcut {
            key: key.to_string(),
            modifiers: modifiers.iter().map(|x| x.to_string()).collect(),
            kbd_shortcut: KeyboardShortcut::new(Modifiers::NONE, Key::A),
        }
    }

    fn build(&mut self, keys: &HashMap<String, Key>) {
        let mut modifiers = Modifiers::default();

        for modi in &self.modifiers {
            match modi.as_str() {
                MOD_ALT => modifiers.alt = true,
                MOD_CTRL => modifiers.ctrl = true,
                MOD_SHIFT => modifiers.shift = true,
                MOD_CMD => modifiers.command = true,
                MOD_MAC_CMD => modifiers.mac_cmd = true,
                _ => {}
            }
        }

        let key = match keys.get(&self.key) {
            Some(k) => k,
            None => return, //uses default unreachable shortcut
        };

        self.kbd_shortcut = KeyboardShortcut {
            logical_key: *key,
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
            sc_toggle_gallery: default_sc_toggle_gallery(),
            sc_exit: default_sc_exit(),
            sc_menu: default_sc_menu(),
            sc_navigator: default_sc_navigator(),
            sc_dir_tree: default_sc_dir_tree(),
            sc_flatten_dir: default_sc_flatten_dir(),
            sc_watch_directory: default_sc_watch_directory(),
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
            scroll_navigation: default_scroll_navigation(),
            user_actions: default_user_actions(),
            context_menu: default_ctx_menu(),
            name_format: default_name_format(),

            sc_fit: default_sc_fit(),
            sc_frame: default_sc_frame(),
            sc_metadata: default_sc_metadata(),
            sc_zoom: default_sc_zoom(),
            sc_next: default_sc_next(),
            sc_prev: default_sc_prev(),
            sc_one_to_one: default_sc_one_to_one(),
            sc_fit_vertical: default_sc_fit_vertical(),
            sc_fit_horizontal: default_sc_fit_horizontal(),
        }
    }
}

impl Default for MultiGalleryConfig {
    fn default() -> Self {
        MultiGalleryConfig {
            images_per_row: default_images_per_row(),
            preloaded_rows: default_preloaded_rows(),
            simultaneous_load: default_simultaneous_load(),
            margin_size: default_margin_size(),
            context_menu: default_ctx_menu(),

            sc_scroll: default_sc_scroll(),
            sc_more_per_row: default_sc_more_per_row(),
            sc_less_per_row: default_sc_less_per_row(),
        }
    }
}

impl Config {
    pub fn new() -> Config {
        let mut cfg = Self::fetch_cfg();
        cfg.build_shortcuts();
        cfg
    }

    pub fn fetch_cfg() -> Config {
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
            Err(e) => {
                println!("{}", e);
                println!("Failure parsing config yaml, using defaults");
                Config::default()
            }
        };

        println!("Using config:");
        println!("{}", serde_yaml::to_string(&cfg).unwrap());

        cfg
    }

    pub fn build_shortcuts(&mut self) {
        let keys = keys();

        //This solution is quite verbose, would be nice to
        //automatically build the shortcut on deserialization.
        self.general.sc_exit.build(&keys);
        self.general.sc_toggle_gallery.build(&keys);
        self.general.sc_menu.build(&keys);
        self.general.sc_navigator.build(&keys);
        self.general.sc_dir_tree.build(&keys);
        self.general.sc_flatten_dir.build(&keys);
        self.general.sc_watch_directory.build(&keys);

        self.gallery.sc_fit.build(&keys);
        self.gallery.sc_frame.build(&keys);
        self.gallery.sc_zoom.build(&keys);
        self.gallery.sc_metadata.build(&keys);
        self.gallery.sc_next.build(&keys);
        self.gallery.sc_prev.build(&keys);
        self.gallery.sc_one_to_one.build(&keys);
        self.gallery.sc_fit_horizontal.build(&keys);
        self.gallery.sc_fit_vertical.build(&keys);

        for action in &mut self.gallery.user_actions {
            action.shortcut.build(&keys);
        }

        self.multi_gallery.sc_scroll.build(&keys);
        self.multi_gallery.sc_more_per_row.build(&keys);
        self.multi_gallery.sc_less_per_row.build(&keys);
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
    Shortcut::from("backspace", &[])
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
pub fn default_sc_metadata() -> Shortcut {
    Shortcut::from("i", &[])
}
pub fn default_sc_zoom() -> Shortcut {
    Shortcut::from("space", &[])
}
pub fn default_sc_next() -> Shortcut {
    Shortcut::from("right", &[])
}
pub fn default_sc_prev() -> Shortcut {
    Shortcut::from("left", &[])
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
pub fn default_margin_size() -> f32 {
    10.
}
pub fn default_sc_scroll() -> Shortcut {
    Shortcut::from("space", &[])
}
pub fn default_sc_more_per_row() -> Shortcut {
    Shortcut::from("plus", &[])
}
pub fn default_sc_less_per_row() -> Shortcut {
    Shortcut::from("minus", &[])
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

fn keys() -> HashMap<String, Key> {
    HashMap::from([
        ("down".to_string(), Key::ArrowDown),
        ("left".to_string(), Key::ArrowLeft),
        ("right".to_string(), Key::ArrowRight),
        ("up".to_string(), Key::ArrowUp),
        ("escape".to_string(), Key::Escape),
        ("tab".to_string(), Key::Tab),
        ("backspace".to_string(), Key::Backspace),
        ("enter".to_string(), Key::Enter),
        ("space".to_string(), Key::Space),
        ("insert".to_string(), Key::Insert),
        ("delete".to_string(), Key::Delete),
        ("home".to_string(), Key::Home),
        ("end".to_string(), Key::End),
        ("pageup".to_string(), Key::PageUp),
        ("pagedown".to_string(), Key::PageDown),
        ("minus".to_string(), Key::Minus),
        ("plus".to_string(), Key::Plus),
        ("0".to_string(), Key::Num0),
        ("1".to_string(), Key::Num1),
        ("2".to_string(), Key::Num2),
        ("3".to_string(), Key::Num3),
        ("4".to_string(), Key::Num4),
        ("5".to_string(), Key::Num5),
        ("6".to_string(), Key::Num6),
        ("7".to_string(), Key::Num7),
        ("8".to_string(), Key::Num8),
        ("9".to_string(), Key::Num9),
        ("a".to_string(), Key::A),
        ("b".to_string(), Key::B),
        ("c".to_string(), Key::C),
        ("d".to_string(), Key::D),
        ("e".to_string(), Key::E),
        ("f".to_string(), Key::F),
        ("g".to_string(), Key::G),
        ("h".to_string(), Key::H),
        ("i".to_string(), Key::I),
        ("j".to_string(), Key::J),
        ("k".to_string(), Key::K),
        ("l".to_string(), Key::L),
        ("m".to_string(), Key::M),
        ("n".to_string(), Key::N),
        ("o".to_string(), Key::O),
        ("p".to_string(), Key::P),
        ("q".to_string(), Key::Q),
        ("r".to_string(), Key::R),
        ("s".to_string(), Key::S),
        ("t".to_string(), Key::T),
        ("u".to_string(), Key::U),
        ("v".to_string(), Key::V),
        ("w".to_string(), Key::W),
        ("x".to_string(), Key::X),
        ("y".to_string(), Key::Y),
        ("z".to_string(), Key::Z),
        ("f1".to_string(), Key::F1),
        ("f2".to_string(), Key::F2),
        ("f3".to_string(), Key::F3),
        ("f4".to_string(), Key::F4),
        ("f5".to_string(), Key::F5),
        ("f6".to_string(), Key::F6),
        ("f7".to_string(), Key::F7),
        ("f8".to_string(), Key::F8),
        ("f9".to_string(), Key::F9),
        ("f10".to_string(), Key::F10),
        ("f11".to_string(), Key::F11),
        ("f12".to_string(), Key::F12),
        ("f13".to_string(), Key::F13),
        ("f14".to_string(), Key::F14),
        ("f15".to_string(), Key::F15),
        ("f16".to_string(), Key::F16),
        ("f17".to_string(), Key::F17),
        ("f18".to_string(), Key::F18),
        ("f19".to_string(), Key::F19),
        ("f20".to_string(), Key::F20),
    ])
}

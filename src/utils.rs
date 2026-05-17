use std::path::Path;

use eframe::egui::{self, Event, Id, Response};
use serde_json::Value;

use crate::VALID_EXTENSIONS;

pub fn textedit_move_cursor_to_end(resp: &Response, ui: &mut egui::Ui, len: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
        let ccursor = egui::text::CCursor::new(len);
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
        state.store(ui.ctx(), resp.id);
        resp.request_focus();
        ui.ctx().memory_mut(|m| m.request_focus(resp.id))
    }
}

pub fn set_mute_state(ctx: &egui::Context, muted: bool) {
    ctx.memory_mut(|mem| {
        mem.data.insert_temp::<bool>(get_muted_data_id(), muted);
    })
}

pub fn are_inputs_muted(ctx: &egui::Context) -> bool {
    ctx.memory_mut(|mem| {
        mem.data
            .get_temp::<bool>(get_muted_data_id())
            .unwrap_or(false)
    }) || ctx.memory(|mem| mem.focused().is_some())
}

pub fn get_muted_data_id() -> Id {
    Id::new("muted")
}

/// Returns true if path contains any images we can open
pub fn is_valid_path(path: &Path) -> bool {
    let dir_info = match path.read_dir() {
        Ok(dir) => dir,
        Err(_) => return false,
    };

    for path in dir_info.flatten() {
        if is_valid_file(&path.path()) {
            return true;
        }
    }

    false
}

pub fn is_valid_file(path: &Path) -> bool {
    VALID_EXTENSIONS.contains(
        &path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
    )
}

pub fn is_invalid_file(path: &Path) -> bool {
    !is_valid_file(path)
}

///Return true if directory starts with '.'
pub fn is_dir_hidden(path: &Path) -> bool {
    path.file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .starts_with('.')
}

pub fn capitalize_first_char(str: &str) -> String {
    let mut chars = str.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

pub fn get_raw_scroll(ctx: &egui::Context) -> f32 {
    let mut delta = 0.0;
    ctx.input(|r| {
        for ev in &r.events {
            match ev {
                Event::MouseWheel {
                    delta: egui::Vec2 { x: _, y },
                    ..
                } => {
                    delta = *y;
                }
                _ => continue,
            }
        }
    });

    delta
}

pub fn get_path_string_without_trailing_slash(path: &Path) -> String {
    let mut path = path;
    if path.is_file() {
        path = if let Some(path) = path.parent() {
            path
        } else {
            return String::new();
        }
    }
    let mut s = path.to_string_lossy().to_string();

    if s.ends_with("/") || s.ends_with("\\") {
        s.pop();
    }

    s
}

pub fn serde_json_value_to_string(value: Value) -> String {
    match value {
        Value::String(s) => s,
        Value::Null => "".to_string(),
        Value::Bool(b) => {
            if b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Array(_) => "".to_string(),
        Value::Object(_) => "".to_string(),
    }
}

pub fn pascal_case_format_space(str: &str) -> String {
    let mut s = String::new();
    let mut prev_upper = false;

    for c in str.chars() {
        if c.is_uppercase() && !s.is_empty() && !prev_upper {
            s.push(' ');
        }

        s.push(c);
        prev_upper = c.is_uppercase();
    }

    s
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::utils::{get_path_string_without_trailing_slash, pascal_case_format_space};

    #[test]
    fn test_get_path_string_without_trailing_slash() {
        assert_eq!(
            "test/data",
            get_path_string_without_trailing_slash(Path::new("test/data/test.txt"))
        );
        assert_eq!(
            "test/data",
            get_path_string_without_trailing_slash(Path::new("test/data/"))
        );
        assert_eq!(
            "test/data",
            get_path_string_without_trailing_slash(Path::new("test/data"))
        );
    }

    #[test]
    fn test_pascal_case_format_space() {
        assert_eq!("Pascal Case", pascal_case_format_space("PascalCase"));
        assert_eq!("Pascal", pascal_case_format_space("Pascal"));
        assert_eq!("pascal", pascal_case_format_space("pascal"));
        assert_eq!("ISO", pascal_case_format_space("ISO"));
    }
}

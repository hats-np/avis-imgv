use std::path::Path;

use eframe::egui::{self, Id, Response};

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
    })
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

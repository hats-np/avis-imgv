use std::path::Path;

use eframe::egui::{self, Id, Response};

use crate::VALID_EXTENSIONS;

pub fn textedit_move_cursor_to_end(resp: &Response, ui: &mut egui::Ui, len: usize) {
    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), resp.id) {
        let ccursor = egui::text::CCursor::new(len);
        state.set_ccursor_range(Some(egui::text::CCursorRange::one(ccursor)));
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
        if VALID_EXTENSIONS.contains(
            &path
                .path()
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string()
                .to_lowercase()
                .as_str(),
        ) {
            return true;
        }
    }

    false
}

///Return true if directory starts with '.'
pub fn is_dir_hidden(path: &Path) -> bool {
    path.file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .starts_with('.')
}

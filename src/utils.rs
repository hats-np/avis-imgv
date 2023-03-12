use eframe::egui::{self, Id, Response};

pub fn textedit_move_cursor_to_end(resp: &Response, ui: &mut egui::Ui, len: usize) {
    let text_edit_id = resp.id;
    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), text_edit_id) {
        let ccursor = egui::text::CCursor::new(len);
        state.set_ccursor_range(Some(egui::text::CCursorRange::one(ccursor)));
        state.store(ui.ctx(), text_edit_id);
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

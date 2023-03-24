use std::{
    path::{Path, PathBuf, MAIN_SEPARATOR_STR},
    str::FromStr,
};

use eframe::{
    egui::{self, Area, Id, Response, Ui},
    epaint::{Color32, Pos2, Shadow},
};

use crate::utils;

pub fn ui(input: &mut String, ctx: &egui::Context) -> bool {
    let mut is_selected = false;
    Area::new("navigator")
        .fixed_pos(Pos2::new(100., 5.))
        .order(egui::Order::Foreground)
        .interactable(true)
        .movable(false)
        .show(ctx, |ui| {
            egui::Frame::window(ui.style())
                .shadow(Shadow {
                    extrusion: (0.),
                    color: (Color32::from_white_alpha(0)),
                })
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.set_width(700.);

                        let mut suggestions = match get_prev_suggestions(ctx) {
                            Some(suggestions) => suggestions,
                            None => get_suggestions(input),
                        };

                        let editor_resp = ui.add(
                            egui::TextEdit::singleline(input).desired_width(ui.available_width()),
                        );

                        editor_resp.request_focus();

                        let mut selected_index = get_index(ctx);
                        let mut selected_path: Option<String> = None;

                        for (i, suggestion) in suggestions.iter().enumerate() {
                            let sl = ui.selectable_label(selected_index == i, suggestion);
                            if sl.clicked() {
                                selected_path = Some(suggestion.clone());
                            }
                        }

                        if selected_index >= suggestions.len() {
                            selected_index = 0;
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown))
                            && selected_index < suggestions.len() - 1
                        {
                            selected_index += 1;
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) && selected_index > 0 {
                            selected_index -= 1;
                            //Arrow up makes the cursor go back in the input box so we need to
                            //compensate
                            utils::textedit_move_cursor_to_end(&editor_resp, ui, input.len());
                        }

                        if !suggestions.is_empty() && ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
                            selected_path = Some(suggestions[selected_index].clone());
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if selected_index == 0 && utils::is_valid_path(Path::new(&input)) {
                                is_selected = true;
                            } else if !suggestions.is_empty() {
                                selected_path = Some(suggestions[selected_index].clone());
                            }
                        }

                        if editor_resp.changed() || selected_path.is_some() {
                            suggestions = get_suggestions(input);
                        }

                        if let Some(selected_path) = selected_path {
                            select_path(input, selected_path, &editor_resp, ui);
                        }

                        set_index(ctx, selected_index);
                        set_suggestions(ctx, &suggestions);
                    })
                })
        });
    is_selected
}

fn get_data_items_id() -> Id {
    Id::new("navigator_items")
}

fn get_data_index_id() -> Id {
    Id::new("navigator_index")
}

fn get_prev_suggestions(ctx: &egui::Context) -> Option<Vec<String>> {
    ctx.memory_mut(|mem| mem.data.get_temp::<Vec<String>>(get_data_items_id()))
}

fn set_suggestions(ctx: &egui::Context, suggestions: &Vec<String>) {
    ctx.memory_mut(|mem| {
        mem.data
            .insert_temp::<Vec<String>>(get_data_items_id(), suggestions.to_owned());
    })
}

fn get_index(ctx: &egui::Context) -> usize {
    ctx.memory_mut(|mem| {
        let data = mem.data.get_temp::<usize>(get_data_index_id());
        data.unwrap_or(0)
    })
}

fn set_index(ctx: &egui::Context, index: usize) {
    ctx.memory_mut(|mem| {
        mem.data.insert_temp::<usize>(get_data_index_id(), index);
    })
}

fn get_path_strings_from_input(input: &str) -> Option<Vec<String>> {
    let mut path = match PathBuf::from_str(input) {
        Ok(path) => path,
        Err(_) => return None,
    };

    if !path.is_dir() {
        path.pop();
    }

    let dir_info = match path.read_dir() {
        Ok(dir_info) => dir_info,
        Err(_) => return None,
    };

    Some(
        dir_info
            .filter_map(|p| match p {
                Ok(p) => match p.metadata() {
                    Ok(m) => {
                        if m.is_dir() && !utils::is_dir_hidden(&p.path()) {
                            string_from_path(&p.path())
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                },
                Err(_) => None,
            })
            .collect(),
    )
}

fn string_from_path(path: &Path) -> Option<String> {
    let path_str = path.as_os_str().to_str()?;

    if let Ok(path_string) = String::from_str(path_str) {
        Some(path_string)
    } else {
        None
    }
}

fn select_path(input: &mut String, suggestion: String, editor_resp: &Response, ui: &mut Ui) {
    let mut suggestion = suggestion;

    if !suggestion.ends_with(MAIN_SEPARATOR_STR) {
        suggestion += MAIN_SEPARATOR_STR;
    }

    *input = suggestion;

    utils::textedit_move_cursor_to_end(editor_resp, ui, input.len());
}

fn get_suggestions(input: &str) -> Vec<String> {
    let mut suggestions = get_path_strings_from_input(input).unwrap_or_default();
    suggestions.retain(|p| p.contains(input));

    suggestions
}

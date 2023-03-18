use std::{path::PathBuf, process::Command};

use eframe::egui::Response;

use crate::config::ContextMenuEntry;

pub fn execute(exec: &str, path: PathBuf) {
    let mut exec = exec.to_string();

    let parent = match path.parent() {
        Some(parent) => parent,
        None => return,
    };
    if exec.contains("{}") {
        let arg = match path.to_str() {
            Some(arg) => arg,
            None => return,
        };
        exec = exec.replace("{}", arg);
    }
    if exec.contains("{.}") {
        let file_stem = match path.file_stem() {
            Some(stem) => stem,
            None => return,
        };
        let file_path = parent.join(file_stem);
        let arg = match file_path.to_str() {
            Some(arg) => arg,
            None => return,
        };
        exec = exec.replace("{.}", arg);
    }
    if exec.contains("{//}") {
        let arg = match parent.to_str() {
            Some(arg) => arg,
            None => return,
        };
        exec = exec.replace("{//}", arg);
    }

    println!("exec -> {}", exec);
    let mut exec_split = exec.split(' ');

    let cmd = match exec_split.next() {
        Some(cmd) => cmd,
        None => return,
    };

    let mut cmd = Command::new(cmd);

    for arg in exec_split {
        cmd.arg(arg);
    }

    //Show toast with result?
    match cmd.spawn() {
        Ok(_) => {}
        Err(e) => {
            println!("{}", e)
        }
    }
}

pub fn build_context_menu(entries: &Vec<ContextMenuEntry>, response: Response, path: PathBuf) {
    if entries.is_empty() {
        return;
    }

    response.context_menu(|ui| {
        ui.set_max_width(300.);
        for entry in entries {
            if ui.button(&entry.description).clicked() {
                execute(&entry.exec.clone(), path.clone());
                ui.close_menu();
            }
        }
    });
}

use std::{path::Path, process::Command};

use eframe::egui::Response;

use crate::{callback::Callback, config::ContextMenuEntry};

/// Executes command, returns false if command wasn't executed
/// or errored out
pub fn execute(exec: &str, path: &Path) -> bool {
    if exec.is_empty() {
        return true;
    }

    let mut exec = exec.to_string();

    let parent = match path.parent() {
        Some(parent) => parent,
        None => return false,
    };
    if exec.contains("{}") {
        let arg = match path.to_str() {
            Some(arg) => arg,
            None => return false,
        };
        exec = exec.replace("{}", arg);
    }
    if exec.contains("{.}") {
        let file_stem = match path.file_stem() {
            Some(stem) => stem,
            None => return false,
        };
        let file_path = parent.join(file_stem);
        let arg = match file_path.to_str() {
            Some(arg) => arg,
            None => return false,
        };
        exec = exec.replace("{.}", arg);
    }
    if exec.contains("{//}") {
        let arg = match parent.to_str() {
            Some(arg) => arg,
            None => return false,
        };
        exec = exec.replace("{//}", arg);
    }
    if exec.contains("{/}") {
        let arg = match path.file_name() {
            Some(arg) => match arg.to_str() {
                Some(arg) => arg,
                None => return false,
            },
            None => return false,
        };
        exec = exec.replace("{/}", arg);
    }
    if exec.contains("{/.}") {
        let arg = match path.file_stem() {
            Some(arg) => match arg.to_str() {
                Some(arg) => arg,
                None => return false,
            },
            None => return false,
        };
        exec = exec.replace("{/.}", arg);
    }

    println!("exec -> {exec}");
    let exec_split = get_command_args(&exec);
    let mut exec_split = exec_split.iter();

    let cmd = match exec_split.next() {
        Some(cmd) => cmd,
        None => return false,
    };

    let mut cmd = Command::new(cmd);

    for arg in exec_split {
        cmd.arg(arg);
    }

    //Show toast with result?
    //We could return the error instead but we don't care much about it now
    //Provide the error to the user in the future
    match cmd.spawn() {
        Ok(_) => true,
        Err(e) => {
            println!("{e}");
            false
        }
    }
}

pub fn get_command_args(cmd: &str) -> Vec<String> {
    let mut args: Vec<String> = vec![];
    let mut it = cmd.chars();
    let mut current = String::new();
    let mut in_string = false;
    loop {
        let next = it.next();

        if next.is_none() {
            if !current.is_empty() {
                args.push(current.to_string());
            }
            break;
        }

        let next = next.unwrap();

        if next == ' ' && !in_string {
            args.push(current.to_string());
            current = String::new();
            in_string = false;
            continue;
        }

        if next == '\'' {
            in_string = !in_string;
        }

        current += &next.to_string();
    }

    args
}

pub fn show_context_menu(
    entries: &Vec<ContextMenuEntry>,
    response: Response,
    path: &Path,
) -> Option<Callback> {
    if entries.is_empty() {
        return None;
    }

    let mut result: Option<Callback> = None;
    response.context_menu(|ui| {
        ui.set_max_width(300.);
        for entry in entries {
            let button_resp = ui.button(&entry.description);

            if button_resp.clicked() {
                if execute(&entry.exec, path) {
                    result = entry.callback.clone();
                }
                ui.close_menu();
            }
        }
    });

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_command_args_returns_correct_args() {
        let cmd = "mkdir /this/is/a/path";
        let args = get_command_args(cmd);
        assert_eq!(args, vec!["mkdir", "/this/is/a/path"])
    }

    #[test]
    fn get_command_args_with_string_returns_correct_args() {
        let cmd = "bash -c 'mkdir /this/is/a/path && cp file /this/is/a/path'";
        let args = get_command_args(cmd);
        assert_eq!(
            args,
            vec![
                "bash",
                "-c",
                "'mkdir /this/is/a/path && cp file /this/is/a/path'"
            ]
        )
    }
}

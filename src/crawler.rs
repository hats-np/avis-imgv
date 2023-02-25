use std::{
    env, fs,
    path::{Path, PathBuf},
};

use crate::VALID_EXTENSIONS;

//TODO: Does not work if file has spaces in it.
pub fn paths_from_args() -> (Vec<PathBuf>, Option<PathBuf>) {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        return (vec![], None);
    }

    if args.len() == 2 {
        let mut path = PathBuf::from(args[1].clone());

        let current_dir = match env::current_dir() {
            Ok(dir) => dir,
            Err(_) => return (vec![path], None),
        };

        if path.is_dir() {
            if path == PathBuf::from(".") {
                path = current_dir;
            } else if !path.has_root() {
                path = current_dir.join(path.strip_prefix(PathBuf::from(".")).unwrap_or(&path));
            }

            let paths = crawler(&path);
            return (paths, None);
        }

        if !path.has_root() {
            path = current_dir.join(path.strip_prefix(PathBuf::from(".")).unwrap_or(&path));
        }

        let parent = match path.parent() {
            Some(parent) => parent,
            None => return (vec![path], None),
        };

        let paths = crawler(parent);
        return (paths, Some(path));
    }

    let paths = args[1..]
        .iter()
        .filter_map(|x| {
            let path = PathBuf::from(x);
            match !VALID_EXTENSIONS.contains(&path.extension()?.to_str()?.to_lowercase().as_str()) {
                true => Some(path),
                false => None,
            }
        })
        .collect();

    (paths, None)
}

fn crawler(path: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    let dir_info = match fs::read_dir(path) {
        Ok(dir_info) => dir_info,
        Err(e) => {
            println!("Failure reading directory -> {}", e);
            return files;
        }
    };

    for file in dir_info {
        match file {
            Ok(f) => {
                let path = f.path();
                if !VALID_EXTENSIONS.contains(
                    &path
                        .extension()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .as_str(),
                ) {
                    continue;
                }
                files.push(path);
            }
            Err(e) => {
                println!("Failure reading file info -> {}", e);
                continue;
            }
        };
    }

    files
}

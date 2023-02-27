use crate::db;
use regex;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::Instant,
};

//for exiftool, the bigger the chunk the better as the startup time is slow
pub const CHUNK_SIZE: &'static usize = &500;
pub const METADATA_PROFILE_DESCRIPTION: &'static str = "Profile Description";
pub const METADATA_ORIENTATION: &'static str = "Orientation";

pub enum Orientation {
    Normal,
    MirrorHorizontal,
    Rotate180,
    MirrorVertical,
    MirrorHorizontalRotate270,
    Rotate90CW,
    MirrorHorizontalRotate90CW,
    Rotate270CW,
}

impl Orientation {
    pub fn from_orientation_metadata(orientation: &str) -> Orientation {
        match orientation {
            "Horizontal (normal)" => Orientation::Normal,
            "Mirror horizontal" => Orientation::MirrorHorizontal,
            "Rotate 180" => Orientation::Rotate180,
            "Mirror vertical" => Orientation::MirrorVertical,
            "Mirror horizontal and rotate 270 CW" => Orientation::MirrorHorizontalRotate270,
            "Rotate 90 CW" => Orientation::Rotate90CW,
            "Mirror horizontal and rotate 90 CW" => Orientation::MirrorHorizontalRotate90CW,
            "Rotate 270 CW" => Orientation::Rotate270CW,
            _ => Orientation::Normal,
        }
    }
}

pub struct Metadata {}

impl Metadata {
    pub fn cache_metadata_for_images(image_paths: &Vec<PathBuf>) {
        let mut image_paths = image_paths
            .into_iter()
            .map(|p| String::from(p.to_string_lossy()))
            .collect();

        thread::spawn(move || {
            let timer = Instant::now();

            let cached_paths = match db::Db::get_cached_images_by_paths(&image_paths) {
                Ok(cached_paths) => cached_paths,
                Err(e) => {
                    println!(
                        "Failure fetching cached metadata paths, aborting caching process {}",
                        e
                    );
                    return;
                }
            };

            image_paths = image_paths
                .into_iter()
                .filter(|x| !cached_paths.contains(x))
                .collect();

            let chunks: Vec<&[String]> = image_paths.chunks(*CHUNK_SIZE).collect();
            for chunk in chunks {
                let chunk_timer = Instant::now();
                let cmd = Command::new("exiftool")
                    .args(chunk)
                    .stdout(Stdio::piped())
                    .spawn();

                match cmd {
                    Ok(cmd) => match cmd.wait_with_output() {
                        Ok(output) => {
                            Self::parse_exiftool_output(&output);
                        }
                        Err(e) => println!("Error fetching metadata -> {}", e),
                    },
                    Err(e) => println!("Error fetching metadata -> {}", e),
                };
                println!(
                    "Cached metadata chunk containing {} images in {}ms",
                    chunk.len(),
                    chunk_timer.elapsed().as_millis()
                );
            }

            println!(
                "Finished caching metadata for all images in {}ms",
                timer.elapsed().as_millis()
            );
        });
    }

    pub fn parse_exiftool_output(output: &Output) {
        //only panics if regex is invalid, impossible to happen in tested builds
        let re = regex::Regex::new(r"========").unwrap();

        let string_output = String::from_utf8_lossy(&output.stdout);

        let mut metadata_to_insert: Vec<(String, String)> = vec![];
        for image_metadata in re.split(&string_output) {
            if let Some((path, tags)) = Self::parse_exiftool_output_str(&image_metadata) {
                let metadata_json = match serde_json::to_string(&tags) {
                    Ok(json) => json,
                    Err(e) => {
                        println!("Failure serializing metadata into json -> {}", e);
                        continue;
                    }
                };
                metadata_to_insert.push((path, metadata_json))
            }
        }

        match db::Db::insert_files_metadata(metadata_to_insert) {
            Ok(_) => {}
            Err(e) => {
                println!("Failure inserting metadata into db -> {}", e);
            }
        }
    }

    pub fn parse_exiftool_output_str(output: &str) -> Option<(String, HashMap<String, String>)> {
        let lines: Vec<&str> = output.split("\n").collect();
        let file_path = lines.get(0)?;

        if file_path.is_empty() {
            return None;
        }

        let tags = output
            .lines()
            .filter(|x| x != &"" && !x.is_empty() && x.contains(":"))
            .filter_map(|x| {
                let split: Vec<&str> = x.split(":").collect();

                if split.len() < 2 {
                    return None;
                }

                let first = String::from(split[0].trim());
                let last = String::from(split[1..].join(":").trim());

                Some((first, last))
            })
            .collect();

        Some((file_path.trim().to_string(), tags))
    }

    pub fn get_image_metadata(path: &str) -> Option<HashMap<String, String>> {
        match db::Db::get_image_metadata(path) {
            Ok(opt) => match opt {
                Some(data) => return Some(serde_json::from_str(&data).unwrap_or_default()),
                None => {}
            },
            Err(e) => println!("Error fetching image metadata from db -> {}", e),
        };

        println!("Metadata not yet in database, fetching for {}", path);

        //This path is useful for the first files that are opened
        //as the first batch(depending on chunk) still takes a bit of time.

        let cmd = Command::new("exiftool")
            .arg(path)
            .stdout(Stdio::piped())
            .spawn();

        let output = match cmd {
            Ok(cmd) => match cmd.wait_with_output() {
                Ok(output) => output,
                Err(e) => {
                    println!("Error fetching metadata -> {}", e);
                    return None;
                }
            },
            Err(e) => {
                println!("Error fetching metadata -> {}", e);
                return None;
            }
        };

        match Self::parse_exiftool_output_str(String::from_utf8_lossy(&output.stdout).as_ref()) {
            Some((_, metadata)) => Some(metadata),
            None => None,
        }
    }

    pub fn extract_icc_from_image(path: &PathBuf) -> Option<Vec<u8>> {
        let cmd = Command::new("exiftool")
            .arg("-icc_profile")
            .arg("-b")
            .arg(path)
            .stdout(Stdio::piped())
            .spawn();

        match cmd {
            Ok(cmd) => match cmd.wait_with_output() {
                Ok(output) => {
                    return {
                        if output.stdout.len() > 0 {
                            Some(output.stdout)
                        } else {
                            None
                        }
                    }
                }
                Err(e) => {
                    println!("Error fetching image icc -> {}", e);
                    return None;
                }
            },
            Err(e) => {
                println!("Error fetching image icc -> {}", e);
                return None;
            }
        };
    }
}

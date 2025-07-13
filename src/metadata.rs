use crate::db;
use regex::{self, Regex};
use std::sync::mpsc;
use std::{
    collections::HashMap,
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::Instant,
};

//for exiftool, the bigger the chunk the better as the startup time is slow
pub const CHUNK_SIZE: &usize = &500;
pub const METADATA_PROFILE_DESCRIPTION: &str = "Profile Description";
pub const METADATA_ORIENTATION: &str = "Orientation";
pub const METADATA_DIRECTORY: &str = "Directory";
pub const METADATA_DATE: &str = "Date/Time Original";

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
    pub fn cache_metadata_for_images_in_background(image_paths: &[PathBuf]) {
        let image_paths = image_paths.to_vec();
        thread::spawn(move || {
            Self::cache_metadata_for_images(&image_paths);
        });
    }

    pub fn cache_metadata_for_images(image_paths: &[PathBuf]) {
        let timer = Instant::now();

        let mut image_paths = image_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<String>>();

        let cached_paths = match db::Db::get_cached_images_by_paths(&image_paths) {
            Ok(cached_paths) => cached_paths,
            Err(e) => {
                println!("Failure fetching cached metadata paths, aborting caching process {e}");
                return;
            }
        };

        image_paths.retain(|x| !cached_paths.contains(x));

        //A bit of a hack but simpler than diverging code paths
        let single_image_path = if image_paths.len() == 1 {
            Some(&image_paths[0])
        } else {
            None
        };

        let chunks: Vec<&[String]> = image_paths.chunks(*CHUNK_SIZE).collect();
        let total_chunks = chunks.len();
        let mut total_elapsed_time_ms = 0u128;

        println!(
            "Caching a total of {} imgs in {} chunks",
            image_paths.len(),
            total_chunks
        );

        for (i, chunk) in chunks.iter().enumerate() {
            println!("Caching chunk {i} of {}", chunks.len());

            let chunk_timer = Instant::now();

            let (tx, rx) = mpsc::channel();
            let mut handles = vec![];
            //4 threads, should be enough to max a HDD
            //Make configurable to take advantage of SSD speeds
            let chunks: Vec<&[String]> = chunk.chunks(*CHUNK_SIZE / 4).collect();
            for chunk in chunks {
                let tx = tx.clone();
                let chunk = chunk.to_vec();
                let handle = thread::spawn(move || {
                    let cmd = Command::new("exiftool")
                        .args(chunk)
                        .stdout(Stdio::piped())
                        .spawn();

                    match cmd {
                        Ok(cmd) => match cmd.wait_with_output() {
                            Ok(output) => {
                                tx.send(output).unwrap();
                            }
                            Err(e) => println!("Error fetching metadata -> {e}"),
                        },
                        Err(e) => println!("Error fetching metadata -> {e}"),
                    };
                });

                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap(); // Wait for each thread to complete
            }

            drop(tx);

            for output in rx {
                Self::parse_exiftool_output(&output, single_image_path);
            }

            let chunk_elapsed_ms = chunk_timer.elapsed().as_millis();
            total_elapsed_time_ms += chunk_elapsed_ms;
            let processed_chunks_count = (i + 1) as u128;

            println!(
                "Cached metadata chunk containing {} images in {}ms",
                chunk.len(),
                chunk_elapsed_ms
            );

            if processed_chunks_count < total_chunks as u128 {
                let avg_time_per_chunk_ms = total_elapsed_time_ms / processed_chunks_count;
                let remaining_chunks = (total_chunks as u128) - processed_chunks_count;
                let estimated_remaining_ms = avg_time_per_chunk_ms * remaining_chunks;

                let estimated_remaining_seconds = estimated_remaining_ms / 1000;
                let estimated_remaining_minutes = estimated_remaining_seconds / 60;
                let estimated_remaining_seconds_remainder = estimated_remaining_seconds % 60;

                println!(
                    "Estimated time remaining: {estimated_remaining_minutes}m {estimated_remaining_seconds_remainder}s"
                );
            }
        }

        println!(
            "Finished caching metadata for all images in {}ms",
            timer.elapsed().as_millis()
        );
    }

    pub fn parse_exiftool_output(output: &Output, path: Option<&String>) {
        //only panics if regex is invalid, impossible to happen in tested builds
        let re = regex::Regex::new(r"========").unwrap();

        let string_output = String::from_utf8_lossy(&output.stdout);

        let mut metadata_to_insert: Vec<(String, String)> = vec![];
        for image_metadata in re.split(&string_output) {
            if let Some((path, tags)) = Self::parse_exiftool_output_str(image_metadata) {
                let metadata_json = match serde_json::to_string(&tags) {
                    Ok(json) => json,
                    Err(e) => {
                        println!("Failure serializing metadata into json -> {e}");
                        continue;
                    }
                };
                metadata_to_insert.push((path, metadata_json))
            }
        }

        //This is required because exiftool doesn't print the filename
        //When only one image is passed
        if let Some(path) = path {
            metadata_to_insert[0].0 = path.clone()
        }

        match db::Db::insert_files_metadata(metadata_to_insert) {
            Ok(_) => {}
            Err(e) => {
                println!("Failure inserting metadata into db -> {e}");
            }
        }
    }

    pub fn parse_exiftool_output_str(output: &str) -> Option<(String, HashMap<String, String>)> {
        let lines: Vec<&str> = output.split('\n').collect();
        let file_path = lines.first()?;

        if file_path.is_empty() {
            return None;
        }

        let tags = output
            .lines()
            .filter(|x| !x.is_empty() && x.contains(':'))
            .filter_map(|x| {
                let split: Vec<&str> = x.split(':').collect();

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
            Ok(opt) => {
                if let Some(data) = opt {
                    return Some(serde_json::from_str(&data).unwrap_or_default());
                }
            }
            Err(e) => println!("Error fetching image metadata from db -> {e}"),
        };

        println!("Metadata not yet in database, fetching for {path}");

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
                    println!("Failure waiting for exiftool process -> {e}");
                    return None;
                }
            },
            Err(e) => {
                println!("Failure spawning exiftool process -> {e}");
                return None;
            }
        };

        Self::parse_exiftool_output_str(String::from_utf8_lossy(&output.stdout).as_ref())
            .map(|(_, metadata)| metadata)
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
                    if !output.stdout.is_empty() {
                        Some(output.stdout)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    println!("Error fetching image icc -> {e}");
                    None
                }
            },
            Err(e) => {
                println!("Error fetching image icc -> {e}");
                None
            }
        }
    }

    pub fn format_string_with_metadata(input: &str, metadata: &HashMap<String, String>) -> String {
        let mut output = String::from(input);

        let tag_regex = Regex::new("(\\$\\(([^\\(\\)]*#([\\w \\s]*)#[^\\(\\)]*)\\))").unwrap();

        for cap_group in tag_regex.captures_iter(input) {
            //Whole string including  $()
            let expression = match cap_group.get(0) {
                Some(m) => m.as_str(),
                None => continue,
            };

            //Above sring without $()
            let string_to_format = match cap_group.get(2) {
                Some(m) => m.as_str(),
                None => continue,
            };

            //Only the metadata key we need to replace
            let metadata_tag = match cap_group.get(3) {
                Some(m) => m.as_str(),
                None => continue,
            };

            let to_replace = if let Some(metadata_value) = metadata.get(metadata_tag) {
                string_to_format.replace(&format!("#{metadata_tag}#"), metadata_value)
            } else {
                "".to_string()
            };

            output = output.replace(expression, &to_replace);
        }

        output
    }

    pub fn clean_moved_files() {
        //This can be a bit heavy if the user has lots of files. We are talking in the millions
        //though... Highly unlikely. Even with 250k images it will use less than 100MB of ram
        //Either way, not a bad idea to do this in chunks eventually
        let paths = match db::Db::get_all_file_paths() {
            Ok(paths) => paths,
            Err(e) => {
                println!("Failure fetching file paths from the database: {e}");
                return;
            }
        };

        let mut to_delete = vec![];
        for path in paths {
            if !PathBuf::from(&path).exists() {
                println!("{path} no longer exists in the filesystem, marking for deletion");
                to_delete.push(path);
            }
        }

        println!("Found {} files to clean from the database", to_delete.len());

        match db::Db::delete_files_by_paths(&to_delete) {
            Ok(()) => println!("Successfully cleaned moved/removed files from the database."),
            Err(e) => {
                println!("Failure deleting moved files from the database: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_string_with_metadata() {
        let input = "$(#File Name#)$( • ƒ#Aperture#)$( • #Shutter Speed#)$( • #ISO# ISO)";
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("File Name".to_string(), "test.jpg".to_string());
        metadata.insert("Aperture".to_string(), "5.0".to_string());
        metadata.insert("ISO".to_string(), "500".to_string());

        assert_eq!(
            Metadata::format_string_with_metadata(input, &metadata),
            "test.jpg • ƒ5.0 • 500 ISO".to_string()
        );
    }
}

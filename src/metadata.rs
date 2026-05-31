use crate::RAW_EXTENSIONS;
use crate::db::DbRepository;
use crate::utils::{format_stars_from_rating, serde_json_value_to_string};
use core::fmt;
use regex::{self, Regex};
use serde_json::{Map, Value};
use std::error::Error;
use std::path::Path;
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
pub const THREAD_COUNT: &usize = &4;
pub const METADATA_PROFILE_DESCRIPTION: &str = "ProfileDescription";
pub const METADATA_ORIENTATION: &str = "Orientation";
pub const METADATA_DIRECTORY: &str = "Directory";
pub const METADATA_DATE: &str = "DateTimeOriginal";

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

#[derive(PartialEq, Debug, Clone)]
pub enum XMPRating {
    Minus1,
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
}
impl XMPRating {
    pub fn from_rating(rating: i32) -> XMPRating {
        match rating {
            -1 => XMPRating::Minus1,
            0 => XMPRating::Zero,
            1 => XMPRating::One,
            2 => XMPRating::Two,
            3 => XMPRating::Three,
            4 => XMPRating::Four,
            5 => XMPRating::Five,
            _ => XMPRating::Zero,
        }
    }

    pub fn list() -> Vec<XMPRating> {
        vec![
            XMPRating::Minus1,
            XMPRating::Zero,
            XMPRating::One,
            XMPRating::Two,
            XMPRating::Three,
            XMPRating::Four,
            XMPRating::Five,
        ]
    }
}

impl fmt::Display for XMPRating {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let rating_str = match self {
            XMPRating::Minus1 => "-1",
            XMPRating::Zero => "0",
            XMPRating::One => "1",
            XMPRating::Two => "2",
            XMPRating::Three => "3",
            XMPRating::Four => "4",
            XMPRating::Five => "5",
        };
        write!(f, "{}", rating_str)
    }
}

#[derive(Default, Clone)]
pub struct ImageMetadata {
    pub exif_tags: HashMap<String, String>,
    pub rating: Option<i32>,
    pub tags: Option<Vec<String>>,
}

impl ImageMetadata {
    pub fn from(raw_exif: &str, rating: Option<i32>, raw_tags: Option<String>) -> ImageMetadata {
        let mut exif: HashMap<String, String> = HashMap::new();
        let mut tags: Option<Vec<String>> = None;

        match serde_json::from_str::<HashMap<String, String>>(raw_exif) {
            Ok(parsed_exif) => {
                exif = parsed_exif;
            }
            Err(e) => tracing::error!("Deserialization failed: {e} | Raw data: {}", raw_exif),
        };

        if let Some(raw_tags) = raw_tags {
            match serde_json::from_str::<Vec<String>>(&raw_tags) {
                Ok(parsed_tags) => {
                    tags = Some(parsed_tags);
                }
                Err(e) => tracing::error!("Deserialization failed: {e} | Raw data: {}", raw_tags),
            };
        }

        ImageMetadata {
            exif_tags: exif,
            rating,
            tags,
        }
    }
}

pub struct Metadata {}

impl Metadata {
    pub fn cache_metadata_for_images<F>(
        db_repo: &mut DbRepository,
        image_paths: &[PathBuf],
        on_update: F,
    ) where
        F: Fn(String),
    {
        let timer = Instant::now();

        on_update(format!("Caching metadata for {} images", image_paths.len()));

        let mut image_paths = image_paths
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<String>>();

        let cached_paths = match db_repo.get_cached_images_by_paths(&image_paths) {
            Ok(cached_paths) => cached_paths,
            Err(e) => {
                tracing::error!(
                    "Failure fetching cached metadata paths, aborting caching process {e}"
                );
                return;
            }
        };

        tracing::info!(
            "Fetched a total of {} paths which are already cached",
            cached_paths.len()
        );

        image_paths.retain(|x| !cached_paths.contains(x));

        tracing::info!("Retained a total of {} images to cache", image_paths.len());

        let chunks: Vec<&[String]> = image_paths.chunks(*CHUNK_SIZE).collect();
        let total_chunks = chunks.len();
        let mut total_elapsed_time_ms = 0u128;

        tracing::info!(
            "Caching a total of {} imgs in {} chunks",
            image_paths.len(),
            total_chunks
        );

        for (i, chunk) in chunks.iter().enumerate() {
            tracing::info!("Caching chunk {i} of {}", chunks.len());

            let chunk_timer = Instant::now();

            let (tx, rx) = mpsc::channel();
            let mut handles = vec![];
            let chunks: Vec<&[String]> = chunk.chunks(*CHUNK_SIZE / THREAD_COUNT).collect();
            for chunk in chunks {
                let tx = tx.clone();
                let chunk = chunk.to_vec();
                let handle = thread::spawn(move || {
                    let cmd = Command::new("exiftool")
                        .arg("-fast2")
                        .arg("-json")
                        .args(chunk)
                        .stdout(Stdio::piped())
                        .spawn();

                    match cmd {
                        Ok(cmd) => match cmd.wait_with_output() {
                            Ok(output) => {
                                tx.send(output).unwrap();
                            }
                            Err(e) => tracing::error!("Error fetching metadata -> {e}"),
                        },
                        Err(e) => tracing::error!("Error fetching metadata -> {e}"),
                    };
                });

                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap(); // Wait for each thread to complete
            }

            drop(tx);

            for output in rx {
                Self::parse_exiftool_output(db_repo, &output);
            }

            let chunk_elapsed_ms = chunk_timer.elapsed().as_millis();
            total_elapsed_time_ms += chunk_elapsed_ms;
            let processed_chunks_count = (i + 1) as u128;

            tracing::info!(
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

                tracing::info!(
                    "Estimated time remaining: {estimated_remaining_minutes}m {estimated_remaining_seconds_remainder}s"
                );

                on_update(format!(
                    "Caching metadata for {} images. Remaining {}m{}s",
                    remaining_chunks * (*CHUNK_SIZE as u128),
                    estimated_remaining_minutes,
                    estimated_remaining_seconds_remainder
                ));
            }
        }

        tracing::info!(
            "Finished caching metadata for all images in {}ms",
            timer.elapsed().as_millis()
        );
    }

    pub fn update_xmp_metadata_for_images<F>(
        db_repo: &mut DbRepository,
        image_paths: &[PathBuf],
        on_update: F,
    ) where
        F: Fn(String),
    {
        let timer = Instant::now();

        let image_paths = image_paths
            .iter()
            .map(|p| format!("{}.xmp", p.to_string_lossy()))
            .filter(|p| Path::new(p).exists())
            .collect::<Vec<String>>();

        on_update(format!(
            "Updating XMP metadata of {} imgs",
            image_paths.len()
        ));

        let chunks: Vec<&[String]> = image_paths.chunks(*CHUNK_SIZE).collect();
        let total_chunks = chunks.len();
        let mut total_elapsed_time_ms = 0u128;

        tracing::info!(
            "Updating XMP metadata of {} imgs in {} chunks",
            image_paths.len(),
            total_chunks
        );

        for (i, chunk) in chunks.iter().enumerate() {
            tracing::info!("Updating chunk {i} of {}", chunks.len());

            let chunk_timer = Instant::now();

            let (tx, rx) = mpsc::channel();
            let mut handles = vec![];
            let chunks: Vec<&[String]> = chunk.chunks(*CHUNK_SIZE / *THREAD_COUNT).collect();
            for chunk in chunks {
                let tx = tx.clone();
                let chunk = chunk.to_vec();
                let handle = thread::spawn(move || {
                    let cmd = Command::new("exiftool")
                        .arg("-json")
                        .arg("-g")
                        .arg("-ext")
                        .arg("xmp")
                        .arg("-XMP:Rating")
                        .arg("-Keywords")
                        .arg("-HierarchicalSubject")
                        .args(chunk)
                        .stdout(Stdio::piped())
                        .spawn();

                    match cmd {
                        Ok(cmd) => match cmd.wait_with_output() {
                            Ok(output) => {
                                tx.send(output).unwrap();
                            }
                            Err(e) => tracing::error!("Error fetching xmp metadata -> {e}"),
                        },
                        Err(e) => tracing::error!("Error fetching xmp metadata -> {e}"),
                    };
                });

                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap(); // Wait for each thread to complete
            }

            drop(tx);

            for output in rx {
                Self::parse_exiftool_xmp_output(db_repo, &output);
            }

            let chunk_elapsed_ms = chunk_timer.elapsed().as_millis();
            total_elapsed_time_ms += chunk_elapsed_ms;
            let processed_chunks_count = (i + 1) as u128;

            tracing::info!(
                "Updated XMP metadata chunk containing {} images in {}ms",
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

                tracing::info!(
                    "Estimated time remaining: {estimated_remaining_minutes}m {estimated_remaining_seconds_remainder}s"
                );

                on_update(format!(
                    "Updating XMP metadata of {} imgs. Remaining {}m{}s",
                    image_paths.len(),
                    estimated_remaining_minutes,
                    estimated_remaining_seconds_remainder
                ));
            }
        }

        tracing::info!(
            "Finished updating xmp metadata for all images in {}ms",
            timer.elapsed().as_millis()
        );
    }

    pub fn parse_exiftool_output(db_repo: &mut DbRepository, output: &Output) {
        let string_output = String::from_utf8_lossy(&output.stdout);
        let list: Vec<Value> = serde_json::from_str(&string_output).unwrap();

        let metadata_to_insert: Vec<(String, String)> = list
            .iter()
            .filter_map(|x| {
                let mut metadata: Map<String, Value> = serde_json::from_value(x.clone()).ok()?;

                let source_file = metadata.get("SourceFile")?.as_str()?.to_string();

                metadata.retain(|_key, value| !value.is_array());

                let json_str = serde_json::to_string(&metadata).ok()?;

                Some((source_file, json_str))
            })
            .collect();

        match db_repo.insert_files_metadata(&metadata_to_insert) {
            Ok(_) => {}
            Err(e) => {
                let paths: Vec<String> =
                    metadata_to_insert.iter().map(|x| x.0.to_string()).collect();
                tracing::error!("Failure inserting metadata into db -> {e} \n paths {paths:?}");
            }
        }
    }

    pub fn parse_exiftool_xmp_output(db_repo: &mut DbRepository, output: &Output) {
        let string_output = String::from_utf8_lossy(&output.stdout);
        let list: Vec<Value> = if let Ok(json) = serde_json::from_str(&string_output) {
            json
        } else {
            tracing::warn!("Failure deserializing XMP json, likely no XMP sidecar files found");
            return;
        };

        let metadata_to_update: Vec<(String, i32, String)> = list
            .iter()
            .filter_map(|x| {
                let metadata: Map<String, Value> = serde_json::from_value(x.clone()).ok()?;

                let source_file = metadata
                    .get("SourceFile")?
                    .as_str()?
                    .strip_suffix(".xmp")?
                    .to_string();

                let xmp = metadata.get("XMP")?.as_object()?;

                let rating = xmp.get("Rating")?.as_i64()? as i32;

                let tags_value = xmp.get("HierarchicalSubject")?;
                let tags_json_str = serde_json::to_string(tags_value).ok()?;

                Some((source_file, rating, tags_json_str))
            })
            .collect();

        match db_repo.update_files_xmp_metadata(&metadata_to_update) {
            Ok(_) => {}
            Err(e) => {
                let paths: Vec<String> =
                    metadata_to_update.iter().map(|x| x.0.to_string()).collect();
                tracing::error!("Failure inserting metadata into db -> {e} \n paths {paths:?}");
            }
        }
    }

    pub fn get_image_metadata(db_repo: &mut DbRepository, path: &str) -> Option<ImageMetadata> {
        tracing::info!("Fetching metadata for image {} ", path);
        match db_repo.get_image_metadata(path) {
            Ok(opt) => {
                if let Some(data) = opt {
                    return Some(data);
                }
            }
            Err(e) => tracing::error!("Error fetching image metadata from db -> {e}"),
        };

        tracing::info!("Metadata not yet in database, fetching for {path}");

        //This path is useful for the first files that are opened
        //as the first batch(depending on chunk) still takes a bit of time.

        let cmd = Command::new("exiftool")
            .arg("-fast2")
            .arg("-json")
            .arg(path)
            .stdout(Stdio::piped())
            .spawn();

        let output = match cmd {
            Ok(cmd) => match cmd.wait_with_output() {
                Ok(output) => output,
                Err(e) => {
                    tracing::error!("Failure waiting for exiftool process -> {e}");
                    return None;
                }
            },
            Err(e) => {
                tracing::error!("Failure spawning exiftool process -> {e}");
                return None;
            }
        };

        let string_output = String::from_utf8_lossy(&output.stdout);
        let list: Vec<Value> = serde_json::from_str(&string_output).unwrap();

        if list.is_empty() {
            return None;
        }

        let mut metadata: Map<String, Value> =
            serde_json::from_value(list.first().unwrap().clone()).ok()?;
        metadata.retain(|_key, value| !value.is_array());

        let exif_tags = metadata
            .into_iter()
            .map(|(k, v)| (k, serde_json_value_to_string(v)))
            .collect::<HashMap<String, String>>();

        //While we fetch the exif on demand if it is not yet in the database due to
        //image decoding needing some values(icc, rotation) we can defer the xmp
        //metadata to be fetched only from the database
        Some(ImageMetadata {
            exif_tags,
            rating: None,
            tags: None,
        })
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
                    tracing::error!("Error fetching image icc -> {e}");
                    None
                }
            },
            Err(e) => {
                tracing::error!("Error fetching image icc -> {e}");
                None
            }
        }
    }

    pub fn format_string_with_metadata(input: &str, metadata: &ImageMetadata) -> String {
        let mut output = String::from(input);

        let tag_regex = Regex::new("(\\$\\(([^\\(\\)]*#([\\w / \\s]*)#[^\\(\\)]*)\\))").unwrap();

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

            let to_replace = if metadata_tag == "Rating" {
                if metadata.rating.is_some() {
                    string_to_format.replace(
                        &format!("#{metadata_tag}#"),
                        &format_stars_from_rating(metadata.rating),
                    )
                } else {
                    "".to_string()
                }
            } else {
                if let Some(metadata_value) = metadata.exif_tags.get(metadata_tag) {
                    string_to_format.replace(&format!("#{metadata_tag}#"), metadata_value)
                } else {
                    "".to_string()
                }
            };

            output = output.replace(expression, &to_replace);
        }

        output
    }

    pub fn clean_moved_files(db_repo: &mut DbRepository) {
        //This can be a bit heavy if the user has lots of files. We are talking in the millions
        //though... Highly unlikely. Even with 250k images it will use less than 100MB of ram
        //Either way, not a bad idea to do this in chunks eventually
        let paths = match db_repo.get_all_file_paths() {
            Ok(paths) => paths,
            Err(e) => {
                tracing::error!("Failure fetching file paths from the database: {e}");
                return;
            }
        };

        let mut to_delete = vec![];
        for path in paths {
            if !PathBuf::from(&path).exists() {
                tracing::error!("{path} no longer exists in the filesystem, marking for deletion");
                to_delete.push(path);
            }
        }

        tracing::info!("Found {} files to clean from the database", to_delete.len());

        match db_repo.delete_files_by_paths(&to_delete) {
            Ok(()) => tracing::info!("Successfully cleaned moved/removed files from the database."),
            Err(e) => {
                tracing::error!("Failure deleting moved files from the database: {e}");
            }
        }
    }

    pub fn clear_moved_files(db_repo: &mut DbRepository, paths: &[PathBuf]) -> usize {
        let mut paths_to_remove: Vec<PathBuf> = vec![];

        for path in paths.iter() {
            if !path.exists() {
                paths_to_remove.push(path.clone());
            }
        }

        if let Err(e) = db_repo.delete_files_by_paths(&paths_to_remove) {
            tracing::error!("Failure deleting nonexistant files from the database -> {e}");
        }

        paths_to_remove.len()
    }

    pub fn group_raw_pairs(
        paths: &[PathBuf],
        lookup: &mut HashMap<String, Vec<PathBuf>>,
    ) -> Vec<PathBuf> {
        let mut paths = paths.to_vec();
        let mut non_raw: HashMap<String, bool> = HashMap::new();
        lookup.clear();

        for path in &paths {
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_string();
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_lowercase();

            if !RAW_EXTENSIONS.contains(&ext.as_str()) {
                let _ = non_raw.insert(stem, true);
            }
        }

        paths.retain(|path| {
            let ext = path
                .extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or_default()
                .to_lowercase();

            if !RAW_EXTENSIONS.contains(&ext.as_str()) {
                return true;
            } else {
                let stem = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .to_string();

                if non_raw.contains_key(&stem) {
                    if let Some(l) = lookup.get_mut(&stem) {
                        l.push(path.clone());
                    } else {
                        lookup.insert(stem, vec![path.clone()]);
                    }
                    return false;
                }
            }

            true
        });
        paths
    }

    pub fn exiftool_rate_images_xmp(paths: &[PathBuf], rating: i32) -> Result<(), Box<dyn Error>> {
        if paths.is_empty() {
            return Ok(());
        }

        let mut cmd = Command::new("exiftool");
        cmd.arg(format!("-XMP:Rating={}", rating))
            .arg("-efile")
            .arg("-srcfile");

        for path in paths {
            let path = path.to_string_lossy().to_string() + ".xmp";
            cmd.arg(path);
        }

        let mut child = cmd.stdout(Stdio::piped()).spawn()?;
        let result = child.wait()?;
        if result.success() {
            tracing::info!("Successfully applied xmp rating {} to {:?}", rating, paths);
            Ok(())
        } else {
            Err(From::from("Exiftool exited with a non zero status"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_string_with_metadata() {
        let input =
            "$(#Rating# • )$(#File Name#)$( • ƒ#Aperture#)$( • #Shutter Speed#)$( • #ISO# ISO)";
        let mut metadata: HashMap<String, String> = HashMap::new();
        metadata.insert("File Name".to_string(), "test.jpg".to_string());
        metadata.insert("Aperture".to_string(), "5.0".to_string());
        metadata.insert("ISO".to_string(), "500".to_string());

        assert_eq!(
            Metadata::format_string_with_metadata(
                input,
                &ImageMetadata {
                    exif_tags: metadata.clone(),
                    rating: Some(2),
                    tags: None
                }
            ),
            "★★☆☆☆ • test.jpg • ƒ5.0 • 500 ISO".to_string()
        );

        assert_eq!(
            Metadata::format_string_with_metadata(
                input,
                &ImageMetadata {
                    exif_tags: metadata,
                    rating: None,
                    tags: None
                }
            ),
            "test.jpg • ƒ5.0 • 500 ISO".to_string()
        );
    }

    #[test]
    fn test_group_raw_pairs() {
        let mut lookup: HashMap<String, Vec<PathBuf>> = HashMap::new();

        let paths = vec![PathBuf::from("photo1.JPG")];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(result, vec![PathBuf::from("photo1.JPG")]);
        assert!(lookup.is_empty());

        let paths = vec![PathBuf::from("photo1.RAF")];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(result, vec![PathBuf::from("photo1.RAF")]);
        assert!(lookup.is_empty());

        let paths = vec![PathBuf::from("photo1.RAF"), PathBuf::from("photo1.JPG")];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(result, vec![PathBuf::from("photo1.JPG")]);
        assert_eq!(
            lookup.get("photo1"),
            Some(&vec![PathBuf::from("photo1.RAF")])
        );

        lookup.clear();
        let paths = vec![PathBuf::from("photo1.JPG"), PathBuf::from("photo1.RAF")];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(result, vec![PathBuf::from("photo1.JPG")]);
        assert_eq!(
            lookup.get("photo1"),
            Some(&vec![PathBuf::from("photo1.RAF")])
        );

        lookup.clear();
        let paths = vec![
            PathBuf::from("photo1.RAF"),
            PathBuf::from("photo1.JPG"),
            PathBuf::from("photo2.RAF"),
            PathBuf::from("photo2.JPG"),
        ];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(
            result,
            vec![PathBuf::from("photo1.JPG"), PathBuf::from("photo2.JPG")]
        );
        assert_eq!(
            lookup.get("photo1"),
            Some(&vec![PathBuf::from("photo1.RAF")])
        );
        assert_eq!(
            lookup.get("photo2"),
            Some(&vec![PathBuf::from("photo2.RAF")])
        );

        lookup.clear();
        let paths = vec![
            PathBuf::from("photo1.RAF"),
            PathBuf::from("photo1.JPG"),
            PathBuf::from("photo2.JPG"),
            PathBuf::from("photo3.RAF"),
        ];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(
            result,
            vec![
                PathBuf::from("photo1.JPG"),
                PathBuf::from("photo2.JPG"),
                PathBuf::from("photo3.RAF")
            ]
        );
        assert_eq!(
            lookup.get("photo1"),
            Some(&vec![PathBuf::from("photo1.RAF")])
        );
        assert_eq!(lookup.len(), 1);

        lookup.clear();
        let paths = vec![
            PathBuf::from("photo3.RAF"),
            PathBuf::from("photo1.JPG"),
            PathBuf::from("photo2.RAF"),
            PathBuf::from("photo1.RAF"),
        ];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(
            result,
            vec![
                PathBuf::from("photo3.RAF"),
                PathBuf::from("photo1.JPG"),
                PathBuf::from("photo2.RAF")
            ]
        );
        assert_eq!(
            lookup.get("photo1"),
            Some(&vec![PathBuf::from("photo1.RAF")])
        );
        assert_eq!(lookup.len(), 1);

        lookup.clear();
        let paths: Vec<PathBuf> = vec![];
        let result = Metadata::group_raw_pairs(&paths, &mut lookup);
        assert_eq!(result, Vec::<PathBuf>::new());
        assert!(lookup.is_empty());
    }
}

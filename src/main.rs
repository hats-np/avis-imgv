use avis_imgv::app::App;
use avis_imgv::db::Db;
use eframe::NativeOptions;
use std::env;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--import" {
        if args.len() < 3 {
            tracing::error!("Usage: avis-imgv --import <path>");
            return;
        }
        let path_str = &args[2];
        let path = PathBuf::from(path_str);

        if !path.exists() {
            tracing::error!("Error: Path does not exist: {path_str}");
            return;
        }

        tracing::info!("Starting recursive crawl from: {path:?}");
        let image_paths = avis_imgv::crawler::crawl(&path, true);
        tracing::info!("Found {} images. Caching metadata...", image_paths.len());
        match Db::init_db() {
            Ok(_) => {}
            Err(e) => {
                panic!("Failure initializing database {e}");
            }
        }

        avis_imgv::metadata::Metadata::cache_metadata_for_images(&image_paths);
        avis_imgv::metadata::Metadata::clean_moved_files();
        tracing::info!("Metadata caching finished. Exiting.");
        return;
    } else if args.len() > 1 && args[1] == "--help" {
        tracing::info!("Usage:");
        tracing::info!("\t --help");
        tracing::info!("\t --import <path> \n \t\t Imports all images in the directory and sub directories into the database");
        tracing::info!("\t --clean <path> \n \t\t Removes moved/deleted files from the database");
        return;
    } else if args.len() > 1 && args[1] == "--clean" {
        avis_imgv::metadata::Metadata::clean_moved_files();
        return;
    }

    let native_options = eframe::NativeOptions {
        ..NativeOptions::default()
    };

    match eframe::run_native(
        "Avis Image Viewer",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    ) {
        Ok(_) => {}
        Err(e) => tracing::error!("{e}"),
    }
}

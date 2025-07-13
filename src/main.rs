use avis_imgv::app::App;
use avis_imgv::db::Db;
use eframe::NativeOptions;
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--import" {
        if args.len() < 3 {
            eprintln!("Usage: avis-imgv --import <path>");
            return;
        }
        let path_str = &args[2];
        let path = PathBuf::from(path_str);

        if !path.exists() {
            eprintln!("Error: Path does not exist: {path_str}");
            return;
        }

        println!("Starting recursive crawl from: {path:?}");
        let image_paths = avis_imgv::crawler::crawl(&path, true);
        println!("Found {} images. Caching metadata...", image_paths.len());
        match Db::init_db() {
            Ok(_) => {}
            Err(e) => {
                panic!("{}", e);
            }
        }

        avis_imgv::metadata::Metadata::cache_metadata_for_images(&image_paths);
        avis_imgv::metadata::Metadata::clean_moved_files();
        println!("Metadata caching finished. Exiting.");
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
        Err(e) => eprintln!("{e}"),
    }
}

use avis_imgv::app::App;
use eframe::{epaint::Vec2, NativeOptions};

fn main() {
    let native_options = eframe::NativeOptions {
        initial_window_size: Some(Vec2::new(1600.0, 1066.0)), //3:2
        ..NativeOptions::default()
    };

    match eframe::run_native(
        "Avis Image Viewer",
        native_options,
        Box::new(|cc| Box::new(App::new(cc))),
    ) {
        Ok(_) => {}
        Err(e) => println!("{}", e),
    }
}

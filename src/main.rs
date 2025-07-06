use avis_imgv::app::App;
use eframe::NativeOptions;

fn main() {
    let native_options = eframe::NativeOptions {
        ..NativeOptions::default()
    };

    match eframe::run_native(
        "Avis Image Viewer",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    ) {
        Ok(_) => {}
        Err(e) => println!("{e}"),
    }
}

use avis_imgv::app::App;
use eframe::epaint::Vec2;

fn main() {
    let mut native_options = eframe::NativeOptions::default();

    //native_options.maximized = true;
    native_options.initial_window_size = Some(Vec2::new(2200.0, 1238.0));
    match eframe::run_native(
        "Avis Image Viewer",
        native_options,
        Box::new(|cc| Box::new(App::new(cc))),
    ) {
        Ok(_) => {}
        Err(e) => println!("{}", e),
    }
}

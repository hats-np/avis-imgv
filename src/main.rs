use avis_imgv::app::App;
use avis_imgv::db::DbRepository;
use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
use eframe::wgpu::{BackendOptions, Backends, InstanceDescriptor, InstanceFlags, MemoryBudgetThresholds};
use eframe::{
    NativeOptions,
    wgpu::{self},
};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

const DEVICE_LABEL: &str = "avis-imgv-device";

fn main() {
    let mut slideshow = false;
    let mut fullscreen = false;

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let args: Vec<String> = env::args().collect();

    tracing::info!("Starting avis-imgv with args: {}", args.join(","));

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
        let mut repo = DbRepository::new();
        match repo.init_db() {
            Ok(_) => {}
            Err(e) => {
                panic!("Failure initializing database {e}");
            }
        }

        avis_imgv::metadata::Metadata::cache_metadata_for_images(&mut repo, &image_paths);
        avis_imgv::metadata::Metadata::clean_moved_files(&mut repo);
        tracing::info!("Metadata caching finished. Exiting.");
        return;
    }
    if args.len() > 1 && args[1] == "--help" {
        tracing::info!("Usage:");
        tracing::info!("\t --help");
        tracing::info!(
            "\t --slideshow <path> \n \t\t Starts in slideshow mode. Useful as a photoframe, screen saver, etc."
        );
        tracing::info!(
            "\t --import <path> \n \t\t Imports all images in the directory and sub directories into the database"
        );
        tracing::info!("\t --clean <path> \n \t\t Removes moved/deleted files from the database");
        return;
    }
    if args.len() > 1 && args[1] == "--clean" {
        let mut repo = DbRepository::new();
        avis_imgv::metadata::Metadata::clean_moved_files(&mut repo);
        return;
    }
    if args.len() > 1 && args.contains(&"--slideshow".to_string()) {
        slideshow = true;
        tracing::info!("Starting with slideshow enabled");
    }
    if args.len() > 1 && args.contains(&"--fullscreen".to_string()) {
        fullscreen = true;
        tracing::info!("Starting with fullscreen enabled");
    }

    match eframe::run_native(
        "Avis Image Viewer",
        get_native_options(),
        Box::new(|cc| Ok(Box::new(App::new(cc, slideshow, fullscreen)))),
    ) {
        Ok(_) => {}
        Err(e) => tracing::error!("{e}"),
    }
}

//Some low powered pcs like raspberry pis can only handle small texture sizes
//The default for egui w/ wgpu seems to be 8192, which is too high for the
//RPi5 which can only handle 4096,
fn get_native_options() -> NativeOptions {
    let device_descriptor_fn = Arc::new(|adapter: &wgpu::Adapter| {
        let adapter_limits = adapter.limits();

        let limits = wgpu::Limits {
            max_texture_dimension_2d: adapter_limits.max_texture_dimension_2d,
            max_texture_array_layers: adapter_limits.max_texture_array_layers,
            ..adapter_limits.clone()
        };

        tracing::info!(
            "Max 2D texture size: {}",
            adapter_limits.max_texture_dimension_2d
        );

        wgpu::DeviceDescriptor {
            label: Some(DEVICE_LABEL),
            required_limits: limits,
            //required_features: wgpu::Features::TEXTURE_FORMAT_16BIT_NORM,
            ..wgpu::DeviceDescriptor::default()
        }
    });

    NativeOptions {
        wgpu_options: WgpuConfiguration {
            //Fix for slow window resize
            present_mode: wgpu::PresentMode::Immediate,
            desired_maximum_frame_latency: Some(2),
            //End Fix
            wgpu_setup: WgpuSetup::CreateNew(WgpuSetupCreateNew {
                device_descriptor: device_descriptor_fn,
                power_preference: wgpu::PowerPreference::HighPerformance,
                instance_descriptor: InstanceDescriptor 
                { 
                    backends: Backends::all(), 
                    flags: InstanceFlags::empty(),
                    memory_budget_thresholds: MemoryBudgetThresholds { for_device_loss: None, for_resource_creation: None}, 
                    backend_options: BackendOptions::default(), 
                    display:  None
                },
                display_handle: None,
                native_adapter_selector: None
            }),
            ..Default::default()
        },
        ..Default::default()
    }
}

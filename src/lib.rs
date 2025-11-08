extern crate core;

use eframe::egui;

pub mod app;
pub mod callback;
pub mod config;
pub mod crawler;
pub mod db;
pub mod dropdown;
pub mod filters;
pub mod gallery_image;
pub mod grid_view;
pub mod icc;
pub mod image;
pub mod image_view;
pub mod metadata;
pub mod navigator;
pub mod perf_metrics;
pub mod theme;
pub mod thumbnail_image;
pub mod tree;
pub mod user_action;
pub mod utils;
pub mod worker;

pub const QUALIFIER: &str = "com";
pub const ORGANIZATION: &str = "avis-imgv";
pub const APPLICATION: &str = "avis-imgv";
pub const JXL_EXTENSION: &str = "jxl";
//RAW EXTENSIONS
pub const RAF_EXTENSION: &str = "raf";
pub const FR3_EXTENSION: &str = "3fr";
pub const ARI_EXTENSION: &str = "ari";
pub const ARQ_EXTENSION: &str = "arq";
pub const ARW_EXTENSION: &str = "arw";
pub const CAM_EXTENSION: &str = "cam";
pub const CR2_EXTENSION: &str = "cr2";
pub const CR3_EXTENSION: &str = "cr3";
pub const CRW_EXTENSION: &str = "crw";
pub const DCR_EXTENSION: &str = "dcr";
pub const DNG_EXTENSION: &str = "dng";
pub const ERF_EXTENSION: &str = "erf";
pub const FFF_EXTENSION: &str = "fff";
pub const GPR_EXTENSION: &str = "gpr";
pub const IIQ_EXTENSION: &str = "iiq";
pub const KDC_EXTENSION: &str = "kdc";
pub const LRI_EXTENSION: &str = "lri";
pub const MDC_EXTENSION: &str = "mdc";
pub const MEF_EXTENSION: &str = "mef";
pub const MOS_EXTENSION: &str = "mos";
pub const MRW_EXTENSION: &str = "mrw";
pub const NEF_EXTENSION: &str = "nef";
pub const NRW_EXTENSION: &str = "nrw";
pub const ORF_EXTENSION: &str = "orf";
pub const ORI_EXTENSION: &str = "ori";
pub const PEF_EXTENSION: &str = "pef";
pub const RAW_EXTENSION: &str = "raw";
pub const RW2_EXTENSION: &str = "rw2";
pub const RWL_EXTENSION: &str = "rwl";
pub const SR2_EXTENSION: &str = "sr2";
pub const SRF_EXTENSION: &str = "srf";
pub const SRW_EXTENSION: &str = "srw";
pub const STI_EXTENSION: &str = "sti";
pub const TIF_EXTENSION: &str = "tif";
pub const X3F_EXTENSION: &str = "x3f";
pub const RAW_EXTENSIONS: &[&str] = &[
    RAF_EXTENSION,
    FR3_EXTENSION,
    ARI_EXTENSION,
    ARQ_EXTENSION,
    ARW_EXTENSION,
    CAM_EXTENSION,
    CR2_EXTENSION,
    CR3_EXTENSION,
    CRW_EXTENSION,
    DCR_EXTENSION,
    DNG_EXTENSION,
    ERF_EXTENSION,
    FFF_EXTENSION,
    GPR_EXTENSION,
    IIQ_EXTENSION,
    KDC_EXTENSION,
    LRI_EXTENSION,
    MDC_EXTENSION,
    MEF_EXTENSION,
    MOS_EXTENSION,
    MRW_EXTENSION,
    NEF_EXTENSION,
    NRW_EXTENSION,
    ORF_EXTENSION,
    ORI_EXTENSION,
    PEF_EXTENSION,
    RAW_EXTENSION,
    RW2_EXTENSION,
    RWL_EXTENSION,
    SR2_EXTENSION,
    SRF_EXTENSION,
    SRW_EXTENSION,
    STI_EXTENSION,
    TIF_EXTENSION,
    X3F_EXTENSION,
];
//RAW EXTENSIONS END
pub const VALID_EXTENSIONS: &[&str] = &[
    "jpg",
    "png",
    "jpeg",
    "webp",
    "gif",
    "bmp",
    "tiff",
    JXL_EXTENSION,
    RAF_EXTENSION,
    FR3_EXTENSION,
    ARI_EXTENSION,
    ARQ_EXTENSION,
    ARW_EXTENSION,
    CAM_EXTENSION,
    CR2_EXTENSION,
    CR3_EXTENSION,
    CRW_EXTENSION,
    DCR_EXTENSION,
    DNG_EXTENSION,
    ERF_EXTENSION,
    FFF_EXTENSION,
    GPR_EXTENSION,
    IIQ_EXTENSION,
    KDC_EXTENSION,
    LRI_EXTENSION,
    MDC_EXTENSION,
    MEF_EXTENSION,
    MOS_EXTENSION,
    MRW_EXTENSION,
    NEF_EXTENSION,
    NRW_EXTENSION,
    ORF_EXTENSION,
    ORI_EXTENSION,
    PEF_EXTENSION,
    RAW_EXTENSION,
    RW2_EXTENSION,
    RWL_EXTENSION,
    SR2_EXTENSION,
    SRF_EXTENSION,
    SRW_EXTENSION,
    STI_EXTENSION,
    TIF_EXTENSION,
    X3F_EXTENSION,
];
pub const SKIP_ORIENT_EXTENSIONS: &[&str] = &[JXL_EXTENSION];

pub const WORKER_MESSAGE_MEMORY_KEY: &str = "worker-message";
pub const FRAME_MEMORY_KEY: &str = "frame-memory";

pub fn no_icon(
    _ui: &egui::Ui,
    _rect: egui::Rect,
    _visuals: &egui::style::WidgetVisuals,
    _is_open: bool,
    _above_or_below: egui::AboveOrBelow,
) {
}

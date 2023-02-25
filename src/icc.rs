pub const SRGB_V4: &[u8; 1232] = include_bytes!("../icc/sRGB-elle-V4-g22.icc");
pub const DISPLAY_P3: &[u8; 536] = include_bytes!("../icc/Display P3.icc");
pub const CLAY_RGB: &[u8; 1276] = include_bytes!("../icc/ClayRGB-elle-V2-g22.icc");

pub const PROFILE_NAME_TO_ICC: &'static [(&'static str, &[u8])] = &[
    ("adobe rgb", CLAY_RGB),
    ("display p3", DISPLAY_P3),
    ("srgb", SRGB_V4),
];

pub fn profile_desc_to_icc(desc: &str) -> Option<&[u8]> {
    for (name, icc) in PROFILE_NAME_TO_ICC {
        if desc.to_lowercase().contains(name) {
            return Some(icc);
        }
    }

    None
}

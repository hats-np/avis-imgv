# avis-imgv

avis-imgv is a fast, configurable and color managed image viewer built with Rust and [egui](https://github.com/emilk/egui). My goal was for it to be fast and to be able to adapt to any kind of hardware power through user configuration.

As of now it's only been tested in Linux but I don't see why it wouldn't work in Windows/MacOS. Configuration and cache directories are obtained through the `directories` crate which is platform agnostic.

[Changelog](docs/changelog.md)

## Dependencies

- coreutils (for installation)
- sqlite
- exiftool
- libwebp for WebP
- libdav1d for AVIF if you enable it in cargo.toml

## Build

With rust [installed](https://rustup.rs/) simply run:

`cargo build --release`

## Install

Take a look at the `install.sh` script. Works in most systems but might need to be adapted. It's still in a rudimentary state and untested in most systems. Linux only for now.

## Color Management

Color management is done through [qcms](https://github.com/FirefoxGraphics/qcms).

Currently avis-imgv is shipped with three(sRGB, Adobe RGB and Display P3) profiles. A profile is chosen based on the exiftool tag "Profile Description" through a `contains` function. This is pretty lax as we can match more specific profiles like `RT_sRGB` with srgb. Open to suggestions on this behaviour. If no profile is matched an extraction will be attempted although it isn't optimal for maximum performance. For this reason it is suggested opening a PR with additional profiles.

Output Profile is sRGB by default and only supports built in profiles. If you need extra profiles either open a PR or edit `icc.rs` and add whichever ones you need for your local builds. It can be configured in `config.yaml`.

sRGB and Adobe RGB(ClayRGB) were taken from [elles_icc_profiles](https://github.com/ellelstone/elles_icc_profiles).

## Supported Image Formats.

Supported image formats can be found [here](https://github.com/image-rs/image/blob/master/README.md) and [here](https://docs.rs/crate/image/latest/features).

Default feature flag for the `image` crate is used by default.

## Planned Features

- Allow to sort by name or date
- Theme Configuration

## User Actions and Context Menu

avis-imgv supports adding user actions, both with a shortcut or a context menu when right clicking on an image.
User actions are simple commands which will be spawned and take in parameters.

Currently three parameters are supported:

- {} Full path
- {.} Path without extension
- {//} Parent path (without last slash)

It is recommended to use simple commands. If you need more complex behaviour, you can use a script and pass the path as a param.

#### Examples

- 'gimp {}' - Opens the file in GIMP.
- 'darktable {.}.RAF' - Opens adjacent Fujifilm raw file in darktable. This one will work best with a script that checks if the file exists.
- 'rate.sh {.}.RAF 5' - Run script which writes a base xmp with image rating. Provided in the examples folder.

#### Callbacks

After successfully executing a user action, we can chose to automatically run a function by specifying a callback. For this just add an entry under either a context menu or a user action. An example is provided under the example config.

- Pop - Removes the selected image from the collection
- Reload - Reloads the selected image
- ReloadAll - Reloads the entire collection
 
## Configuration

Configuration file should be: `~/.config/avis-imgv/config.yaml`. An example is provided under examples/config.yaml.

### General

Keys | Values | Default 
--- | --- | ---
limit_cached  | Maximum number of cached files metadata | 100000
output_icc_profile | Output icc profile | srgb 
text_scaling | Text Scaling | 1.25

### Gallery

Keys | Values | Default 
--- | --- | ---
loaded_images | Number of loaded images in each direction. Adjust based on how much RAM you want to use. | 5
should_wait | Should wait for image to finish loading before advancing to the next one | true
metadata_tags | Metadata visible in the Image Information side pannel(when opened) | Date/Time Original, Created Date, Camera Model Name, Lens Model, Focal Length, Aperture Value, Exposure Time, ISO, Image Size, Color Space, Directory
frame_size_relative_to_image | White frame size relative to smallest image side | 0.2
scroll_navigation | Should scroll be used for navigation | true
name_format | Format for file name in bottom bar. Uses `$(#exif_tag#)` expressions. If exif tag is not found the entire expression will be ignored. Ex: `$(#File Name#)$( • ƒ#Aperture#)$( • #Shutter Speed#)$( • #ISO# ISO)` -> `DSCF6114.JPG • ƒ5.6 • 1/500 • 200 ISO`

### Multi Gallery

Keys | Values | Default 
--- | --- | ---
images_per_row | How many images should be displayed per row | 3
preloaded_rows | How many off-screen rows in each direction should be loaded and remain in memory | 2
simultaneous_load | How many images should be allowed to load at the same time | 8 (Adjust according to core count or how much you want to work your PC)
margin_size | Margin size between images | 10.

## Default Shortcuts

Shortcuts can be configured in the settings. Check examples/config.yaml for a an example and keys.txt for valid keys and modifiers.

### General

Key | Action
--- | --- 
Backspace | Toggle between Single and Multi Gallery
Q | Exit
F1 | Toggles a menu which allows to open a folder or file
Ctrl + L | Shows navigation bar
T | Show Directory Tree

### Single Gallery

Key | Action
--- | --- 
F | Fit image to screen
G | Toggle a white frame around the image
I | Display side tab with image metadata
Spacebar | Toggle Zoom
Ctrl+Scroll | Zoom image
Scroll | Next or Previous 
Arrow Keys | Next or Previous

### Multi Gallery
Key | Action
--- | ---
Spacebar | Scroll down
Double Click | Open Single gallery on selected image
Ctrl+Scroll | Increase/Decrease nr of images per row
\+ | Increase nr of images per row
\- | Decrease nr of images per row


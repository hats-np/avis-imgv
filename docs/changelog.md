# Change Log

## 2024-09-03

- Configuration file now uses json. This allows us to drop our dependency on yaml serialziation.
- Upgraded most crates to their recent version.
- QOL improvements and small bugfixes.
- Added a "latch maximize" function(ctrl + m) that automatically extends images to their maximum size for the screen.
- Added a maximize shortcut (m).
- Improved shortcuts code wise, this will allow easier configurations in the future.

## 2023-11-25

- Added the ability to watch the opened directory for new files or file changes. Can work together with directory flattening to watch all sub directories recursively. Shortcut is `sc_watch_directory` under general.
- Added sort by file modification date.
- Upgraded egui to latest version.
- Upgraded zune jpeg to latest version.

## 2023-09-32

- Added the ability to flatten the open directory, reading all files from subdirectories. Shortcut `sc_flatten_dir` under general.
- Allow to sort by random.

## 2023-04-26

### Added

- Right click menu for image magnification. Shortcut to set magnification as one to one (100%). Shortcut `sc_one_to_one`
  under `gallery`.
- Right click menu on magnification now also has "fit to screen", "fit horizontal" and "fit vertical". Shortcut
  `sc_fit_horizontal` and `sc_fit_vertical` under `gallery`.

## 2023-03-29

All config keys in the root of the file will need to be put under a "general" config. Please check the example
configuration.
"sc_del" and "delete_cmd" configs were removed as it's prefered to do it using a delete command plus a callback.

### Added

- User actions and Context Menus now can have callbacks. Currently 3 were implemented: Pop, Reload and ReloadAll.

### Changed

- Various bugfixes and adjustments.
- qcms now pulls directly from its repo as the crate is outdated and requires rust bootstrap.

---

## 2023-03-19

Two new configuration entries were added, "sc_dir_tree".

### Added

- Added a directory tree pannel to quickly browser through directories.

### Changed

### Fixed

---

## 2023-03-18

Two new configuration entries were added, "sc_del" and "delete_cmd" both under gallery.

### Added

- Added a shortcut to delete/move files. It executes the configured command, removes the image from the current list and
  loads the next image.

### Changed

- Implemented the fast_image_resize crate and changed the resizing algorithm to Bilinear. This greatly improves multi
  gallery performance.

### Fixed

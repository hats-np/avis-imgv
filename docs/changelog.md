# Change Log

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

- Added a shortcut to delete/move files. It executes the configured command, removes the image from the current list and loads the next image.

### Changed

- Implemented the fast_image_resize crate and changed the resizing algorithm to Bilinear. This greatly improves multi gallery performance.
 
### Fixed

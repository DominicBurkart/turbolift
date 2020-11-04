#[cfg(target_family = "unix")]
pub use std::os::unix::fs::symlink as symlink_dir;

#[cfg(target_family = "windows")]
pub use std::os::windows::fs::symlink_dir;

#[cfg(not(debug_assertions))]
pub const IS_RELEASE: bool = true;

#[cfg(debug_assertions)]
pub const IS_RELEASE: bool = false;

/// is --release if built with release flag, otherwise empty string
pub const RELEASE_FLAG: &str = {
    if IS_RELEASE {
        "--release"
    } else {
        ""
    }
};

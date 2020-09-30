#[cfg(target_family = "unix")]
pub use std::os::unix::fs::symlink as symlink_dir;

#[cfg(target_family = "windows")]
pub use std::os::windows::fs::symlink_dir;

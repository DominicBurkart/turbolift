use std::net::UdpSocket;

#[cfg(target_family = "unix")]
pub use std::os::unix::fs::symlink as symlink_dir;

#[cfg(target_family = "windows")]
pub use std::os::windows::fs::symlink_dir;

pub fn get_open_socket() -> std::io::Result<UdpSocket> {
    UdpSocket::bind("127.0.0.1:0")
}

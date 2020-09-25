#[macro_use]
extern crate lazy_static;
use std::path::Path;

pub mod build_project;
pub mod distributed_platform;
pub mod extract_function;
pub mod utils;
pub mod local_queue;
pub mod kubernetes;
pub use serde_json;

lazy_static! {
    /// CACHE_PATH is the directory where turbolift stores derived projects,
    /// their dependencies, and their build artifacts. Each distributed
    /// function has its own project subdirectory in CACHE_PATH.
    pub static ref CACHE_PATH: &'static Path  = Path::new(".turbolift");
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

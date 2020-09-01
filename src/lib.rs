#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use std::fs;
use std::path::Path;

use tar::Builder;
use quote::quote;

pub mod distributed_platform;
pub mod local_queue;
pub mod extract_function;
pub mod build_project;
pub use crate::distributed_platform::{DistributionPlatform, DistributionError, DistributionResult};

lazy_static! {
    /// CACHE_PATH is the directory where turbolift stores derived projects,
    /// their dependencies, and their build artifacts. Each distributed
    /// function has its own project subdirectory in CACHE_PATH.
    pub static ref CACHE_PATH: &'static Path  = Path::new(".turbolift");
}
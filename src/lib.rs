#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use std::fs;

use tar::Builder;
use quote::quote;

pub mod distributed_platform;
pub mod local_queue;
pub mod extract_function;
pub mod build_project;
pub use crate::extract_function::CACHE_PATH;



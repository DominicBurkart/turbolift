use std::path::Path;
use proc_macro2::TokenStream;
use proc_macro;
use std::io::Cursor;
use std::fs;

use tar::{Builder, Archive};

use crate::distributed_platform::DistributionResult;

lazy_static! {
    /// This is an example for using doc comment attributes
    pub static ref CACHE_PATH: &'static Path  = Path::new("turbolift");
}

pub fn get_typed_params(function: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn to_untyped_params(typed_params: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn to_path_params(untyped_params: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn to_param_types(typed_params: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn get_function_name(function: &TokenStream) -> String {
    unimplemented!()
}

pub fn get_result_type(function: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn params_json_vec(param_types: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn get_sanitized_file(function: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn unpack_path_params(untyped_params: &TokenStream) -> TokenStream {
    unimplemented!()
}

pub fn make_compressed_proj_src(dir: &Path) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    let mut archive = Builder::new(cursor);
    archive.append_dir_all("", dir).unwrap();
    archive.finish().unwrap();
    archive
        .into_inner()
        .unwrap()
        .into_inner()
}

pub fn decompress_proj_src(src: Vec<u8>, dest: &Path) -> DistributionResult<()> {
    let cursor = Cursor::new(src);
    let mut archive = Archive::new(cursor);
    Ok(archive.unpack(dest)?)
}

pub fn bin_vector_to_literal_tokens(vector: Vec<u8>) -> TokenStream {
    let mut literal = String::new();
    literal.push_str("vec![");
    let mut first = true;
    for value in vector {
        if first {
            first = false;
        } else {
            literal.push_str(",");
        }
        literal.push_str(&value.to_string());
    }
    literal.parse().unwrap()
}

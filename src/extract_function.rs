use std::path::Path;
use proc_macro2::TokenStream;
use proc_macro;
use std::io::Cursor;
use std::fs;
use std::str::FromStr;

use tar::{Builder, Archive};
use syn;
use quote::ToTokens;

use crate::distributed_platform::DistributionResult;
use syn::spanned::Spanned;
use syn::export::TokenStream2;

type TypedParams = syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]>;
type UntypedParams = syn::punctuated::Punctuated<Box<syn::Pat>, syn::Token![,]>;
type ParamTypes = syn::punctuated::Punctuated<Box<syn::Type>, syn::Token![,]>;

lazy_static! {
    /// CACHE_PATH is the directory where turbolift stores derived projects,
    /// their dependencies, and their build artifacts. Each distributed
    /// function has its own project subdirectory in CACHE_PATH.
    pub static ref CACHE_PATH: &'static Path  = Path::new(".turbolift");
}

pub fn get_fn_signature(function: TokenStream) -> syn::Signature {
    match syn::parse2(function).unwrap() {
        syn::Item::Fn(fn_item) => {
            fn_item.sig
        },
        _ => panic!("token stream does not represent function.")
    }
}

pub fn to_untyped_params(
    typed_params: TypedParams
) -> UntypedParams {
    typed_params
        .into_iter()
        .map(
            |fn_arg| match fn_arg {
                syn::FnArg::Receiver(_r) => panic!("[to_untyped_params] receiver passed"),
                syn::FnArg::Typed(pat_type) => pat_type.pat
            }
        )
        .collect()
}
/// params -> {param1}/{param2}/{param3}[...]
pub fn to_path_params(
    untyped_params: UntypedParams
) -> String {
    let open_bracket =  "{";
    let close_bracket = "}".to_string();
    let path_format: Vec<String> = untyped_params
        .into_iter()
        .map(
            |pat|
                open_bracket.to_string() + &pat.into_token_stream().to_string() + &close_bracket
        )
        .collect();

    path_format.join("/")
}

pub fn to_param_types(
    typed_params: TypedParams
) -> ParamTypes {
    typed_params
        .into_iter()
        .map(
            |fn_arg| match fn_arg {
                syn::FnArg::Receiver(_r) => panic!("[to_untyped_params] receiver passed"),
                syn::FnArg::Typed(pat_type) => pat_type.ty
            }
        )
        .collect()
}

pub fn params_json_vec(
    untyped_params: UntypedParams
) -> TokenStream {
    let punc: Vec<String> = untyped_params
        .into_iter()
        .map(
            |pat|
                "serde_json::to_string(".to_string() + &pat.into_token_stream().to_string() + ")"
        )
        .collect();

    let vec_string = format!(
        "vec![{}]",
        punc.join(", ")
    );
    TokenStream::from_str(&vec_string).unwrap()
}

pub fn get_sanitized_file(function: &TokenStream) -> TokenStream {
    let span = function.span();
    let path = span.source_file().path();
    let start_line = span.start().line -2; // todo HACK func def can be more than one line
    let end_line = span.end().line;

    // generate a file with everything
    let file_contents = std::fs::read_to_string(path).unwrap();

    // remove targeted function definition
    let file_contents_without_target_function = {
        type Line = String;
        let mut file_lines: Vec<Line> = file_contents
            .lines()
            .map(|v| v.to_string())
            .collect();
        println!("draining lines: {}..{}", start_line, end_line);
        println!("{}", file_lines.len());
        file_lines.drain(start_line..end_line);
        println!("{}", file_lines.len());
        file_lines
            .join("\n")
    };

    // remove main function if it exists
    let sanitized_string = {
        // todo handle if the main function is decorated
        // todo remove main function instead of just mangling it
        let main_definition = "fn main()";
        file_contents_without_target_function.replace(main_definition, "fn _super_main()")
    };
    println!("sanitized string: {}", sanitized_string);
    TokenStream2::from_str(&sanitized_string).unwrap()
}

pub fn unpack_path_params(untyped_params: &UntypedParams) -> TokenStream {
    let n_params = untyped_params.len();
    let params: Vec<String> = (0..n_params)
        .map(|i| format!("path.{}", i))
        .collect();
    TokenStream::from_str(&params.join(", ")).unwrap()
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

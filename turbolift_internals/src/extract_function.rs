use proc_macro2::TokenStream;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use quote::ToTokens;
use tar::{Archive, Builder};

use syn::export::TokenStream2;
use syn::spanned::Spanned;

use crate::distributed_platform::DistributionResult;
use crate::CACHE_PATH;
use std::collections::VecDeque;

type TypedParams = syn::punctuated::Punctuated<syn::FnArg, syn::Token![,]>;
type UntypedParams = syn::punctuated::Punctuated<Box<syn::Pat>, syn::Token![,]>;
type ParamTypes = syn::punctuated::Punctuated<Box<syn::Type>, syn::Token![,]>;

pub fn get_fn_item(function: TokenStream) -> syn::ItemFn {
    match syn::parse2(function).unwrap() {
        syn::Item::Fn(fn_item) => fn_item,
        _ => panic!("token stream does not represent function."),
    }
}

/// wraps any calls to the target function from within its own service with the return type as
/// if the call was made from outside the service. This is one way to allow compilation while
/// references to the target function are in the service codebase.
pub fn make_dummy_function(
    function: syn::ItemFn,
    redirect_fn_name: &str,
    untyped_params: UntypedParams,
) -> syn::ItemFn {
    let redirect_statement: syn::Stmt = syn::parse_str(&format!(
        "return Ok({}({}));",
        redirect_fn_name,
        untyped_params.to_token_stream().to_string()
    ))
    .unwrap();
    let output = match function.sig.output {
        syn::ReturnType::Default => syn::ReturnType::Type(
            Default::default(),
            Box::new(syn::Type::Verbatim(TokenStream2::from_str("()").unwrap())),
        ),
        syn::ReturnType::Type(arrow_token, return_box) => syn::ReturnType::Type(
            arrow_token,
            Box::new(syn::Type::Verbatim(
                TokenStream2::from_str(&format!(
                    "turbolift::DistributionResult<{}>",
                    return_box.to_token_stream().to_string()
                ))
                .unwrap(),
            )),
        ),
    };
    syn::ItemFn {
        block: Box::new(syn::Block {
            brace_token: syn::token::Brace {
                span: redirect_statement.span(),
            },
            stmts: vec![redirect_statement],
        }),
        sig: syn::Signature {
            asyncness: Some(Default::default()),
            output,
            ..function.sig
        },
        ..function
    }
}

pub fn to_untyped_params(typed_params: TypedParams) -> UntypedParams {
    typed_params
        .into_iter()
        .map(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_r) => panic!("[to_untyped_params] receiver passed"),
            syn::FnArg::Typed(pat_type) => pat_type.pat,
        })
        .collect()
}
/// params -> {param1}/{param2}/{param3}[...]
pub fn to_path_params(untyped_params: UntypedParams) -> String {
    let open_bracket = "{";
    let close_bracket = "}".to_string();
    let path_format: Vec<String> = untyped_params
        .into_iter()
        .map(|pat| open_bracket.to_string() + &pat.into_token_stream().to_string() + &close_bracket)
        .collect();

    path_format.join("/")
}

pub fn to_param_types(typed_params: TypedParams) -> ParamTypes {
    typed_params
        .into_iter()
        .map(|fn_arg| match fn_arg {
            syn::FnArg::Receiver(_r) => panic!("[to_untyped_params] receiver passed"),
            syn::FnArg::Typed(pat_type) => pat_type.ty,
        })
        .collect()
}

pub fn params_json_vec(untyped_params: UntypedParams) -> TokenStream {
    let punc: Vec<String> = untyped_params
        .into_iter()
        .map(|pat| {
            "turbolift::serde_json::to_string(&".to_string()
                + &pat.into_token_stream().to_string()
                + ").unwrap()"
        })
        .collect();

    let vec_string = format!("vec![{}]", punc.join(", "));
    TokenStream::from_str(&vec_string).unwrap()
}

pub fn get_sanitized_file(function: &TokenStream) -> TokenStream {
    let span = function.span();
    let path = span.source_file().path();
    let start_line = span.start().line - 2; // todo HACK func def can be more than one line
    let end_line = span.end().line;

    // generate a file with everything
    let file_contents = std::fs::read_to_string(path).unwrap();

    // remove target function
    let target_function_removed = {
        type Line = String;
        let mut file_lines: Vec<Line> = file_contents.lines().map(|v| v.to_string()).collect();
        file_lines.drain(start_line..end_line);
        file_lines.join("\n")
    };

    let sanitized_string = {
        // remove main function if it exists
        // todo handle if the main function is decorated
        // todo remove main function instead of just mangling it
        let main_definition = "fn main()";
        target_function_removed.replace(main_definition, "fn _super_main()")
    };
    TokenStream2::from_str(&sanitized_string).unwrap()
}

pub fn unpack_path_params(untyped_params: &UntypedParams) -> TokenStream {
    let n_params = untyped_params.len();
    let params: Vec<String> = (0..n_params).map(|i| format!("path.{}", i)).collect();
    TokenStream::from_str(&params.join(", ")).unwrap()
}

pub fn make_compressed_proj_src(dir: &Path) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut archive = Builder::new(cursor);

    println!("uwu");

    let mut entries: VecDeque<(PathBuf, std::fs::DirEntry)> = fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| (dir.file_name().unwrap().into(), entry))
        .collect(); // ignore read errors

    archive.append_dir(dir.file_name().unwrap(), dir).unwrap();
    while !entries.is_empty() {
        let (entry_parent, entry) = entries.pop_front().unwrap();
        if entry.file_name().to_str() == Some("target") && entry.metadata().unwrap().is_dir() {
            // in target directories, only pass release (if it exists)
            let release_deps = entry.path().join("release/deps");
            if release_deps.exists() {
                println!("entry target: {:?}", entry.path());
                let path = {
                    if entry_parent == dir {
                        PathBuf::from_str("target/release").unwrap()
                    } else {
                        entry_parent.join("target").join("release")
                    }
                };
                archive.append_dir_all(path, release_deps).unwrap();
            }
        } else {
            let entry_path_with_parent = entry_parent.join(entry.file_name());
            if entry.path().is_dir() {
                // ^ bug: the metadata on symlinks sometimes say that they are not directories,
                // so we have to metadata.is_dir() || (metadata.file_type().is_symlink() && entry.path().is_dir() )
                if CACHE_PATH.file_name().unwrap() != entry.file_name() {
                    archive
                        .append_dir(&entry_path_with_parent, entry.path())
                        .unwrap();
                    entries.extend(
                        fs::read_dir(entry.path())
                            .unwrap()
                            .filter_map(Result::ok)
                            .map(|child| (entry_path_with_parent.clone(), child)),
                    )
                } else {
                    // don't include the cache
                }
            } else {
                println!("entry file: {:?}", entry.path());
                let mut f = fs::File::open(entry.path()).unwrap();
                archive.append_file(entry_path_with_parent, &mut f).unwrap();
            }
        }
    }
    println!("finishin");
    archive.finish().unwrap();
    archive.into_inner().unwrap().into_inner()
}

pub fn decompress_proj_src(src: &[u8], dest: &Path) -> DistributionResult<()> {
    let cursor = Cursor::new(src.to_owned());
    let mut archive = Archive::new(cursor);
    Ok(archive.unpack(dest)?)
}

/// assumes input is a function, not a closure.
pub fn get_result_type(output: &syn::ReturnType) -> TokenStream2 {
    match output {
        syn::ReturnType::Default => TokenStream2::from_str("()").unwrap(),
        syn::ReturnType::Type(_right_arrow, boxed_type) => boxed_type.to_token_stream(),
    }
}

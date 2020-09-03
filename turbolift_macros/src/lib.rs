#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use std::fs;
use std::path::{Path, PathBuf};

use tar::Builder;
use quote::quote;
use fs_extra;

use turbolift::{build_project, extract_function};
use turbolift::CACHE_PATH;

#[proc_macro_attribute]
pub fn on(distribution_platform_: TokenStream, function_: TokenStream) -> TokenStream {
    // convert proc_macro::TokenStream to proc_macro2::TokenStream
    let distribution_platform = TokenStream2::from(distribution_platform_);
    let function = TokenStream2::from(function_);

    // generate derived syntax
    let signature = extract_function::get_fn_signature(function.clone());
    let function_name = signature.ident;
    let function_name_string = function_name.to_string();
    let typed_params = signature.inputs;
    let untyped_params = extract_function::to_untyped_params(typed_params.clone());
    let params_as_path = extract_function::to_path_params(untyped_params.clone());
    let param_types = extract_function::to_param_types(typed_params.clone());
    let params_vec = extract_function::params_json_vec(untyped_params.clone());
    let unpacked_path_params = extract_function::unpack_path_params(&untyped_params);
    let result_type = signature.output;
    println!("derived tokens initiated");

    // todo extract any docs from passed function and put into fn wrapper

    // read current file to access imports and local functions
    let sanitized_file = extract_function::get_sanitized_file(&function);
    // todo make code below hygienic in case sanitized_file also imports from actix_web
    let main_file = quote! {
        use actix_web::{web, HttpResponse, Result};
        #sanitized_file

        async fn turbolift_wrapper(path: web::Path<(#param_types)>) -> Result<HttpResponse> {
            Ok(
                HttpResponse::Ok()
                    .json(#function_name(#unpacked_path_params))
            )
        }

        #[actix_rt::main]
        async fn main() -> std::io::Result<()> {
            use actix_web::{App, HttpServer};

            let args: Vec<String> = std::env::args().collect();
            let ip_and_port = &args[1];
            HttpServer::new(
                ||
                    App::new()
                        .route(
                            "/" + #function_name_string + "/" + #params_as_path,
                            web::to(wrapper)
                        )
            )
            .bind(ip_and_port)?
            .run()
            .await
        }
    };

    println!("application code generated: {}", main_file.to_string());

    // copy all files in repo into cache
    let function_cache_proj_path = CACHE_PATH.join(function_name_string.clone());
    fs::create_dir_all(function_cache_proj_path.clone()).unwrap();
    let files_to_copy: Vec<PathBuf> = fs::read_dir(".")
        .unwrap()
        .map(
            |res| res.unwrap().path()
        ).filter(
            |path| !path.to_string_lossy().contains(".turbolift") // todo hack
        ).collect();
    fs_extra::copy_items(
        &files_to_copy,
        function_cache_proj_path.clone(),
        &fs_extra::dir::CopyOptions {
            overwrite: true,
            ..Default::default()
        }
    ).unwrap();

    // edit project main file
    let target_main_file = function_cache_proj_path
        .join("src")
        .join("main.rs");
    fs::write(
        target_main_file,
        main_file.to_string()
    ).unwrap();

    // modify cargo.toml (edit package info & add actix + json_serde deps)
    build_project::edit_cargo_file(
        &function_cache_proj_path.join("cargo.toml"),
        &function_name_string
    );
    println!("source project generated");

    // build project and give helpful compile-time errors
    build_project::make_executable(&function_cache_proj_path, None);
    println!("project built");

    // compress project source files
    let project_source_binary = extract_function::bin_vector_to_literal_tokens(
        extract_function::make_compressed_proj_src(&function_cache_proj_path)
    );
    println!("project compressed");

    let declare_and_dispatch = quote! {
        extern crate turbolift;
        extern crate serde_json;

        // register binary with distribution platform
        #distribution_platform.declare(#function_name_string, #project_source_binary);

        // dispatch call and process response
        async fn #function_name(#typed_params) -> turbolift::distributed_platform::DistributionResult<#result_type> {
            let params = #params_vec.join("/");
            let resp_str = #distribution_platform.dispatch(#function_name_string, params).await?;
            // ^ todo change dispatch to return a stream/iterator
            serde_json::from_str(&resp_str)
        }
    };
    println!("declare and dispatch code generated");
    declare_and_dispatch.into()
}

#[proc_macro_attribute]
pub fn with(_attr: TokenStream, item: TokenStream) -> TokenStream {
    unimplemented!()
}
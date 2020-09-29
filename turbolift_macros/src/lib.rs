use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use quote::{format_ident, quote};

use turbolift_internals::{build_project, extract_function, CACHE_PATH};

#[proc_macro_attribute]
pub fn on(distribution_platform_: TokenStream, function_: TokenStream) -> TokenStream {
    // convert proc_macro::TokenStream to proc_macro2::TokenStream
    let distribution_platform = TokenStream2::from(distribution_platform_);
    let function = TokenStream2::from(function_);

    // generate derived syntax
    let original_target_function = extract_function::get_fn_item(function.clone());
    let original_target_function_ident = original_target_function.sig.ident.clone();
    let original_target_function_name = original_target_function_ident.to_string();
    let mut target_function = original_target_function.clone();
    target_function.sig.ident = format_ident!("{}_raw", target_function.sig.ident);
    let signature = target_function.sig.clone();
    let function_name = signature.ident;
    let function_name_string = function_name.to_string();
    let typed_params = signature.inputs;
    let untyped_params = extract_function::to_untyped_params(typed_params.clone());
    let params_as_path = extract_function::to_path_params(untyped_params.clone());
    let wrapper_route = "/".to_string() + &original_target_function_name + "/" + &params_as_path;
    let param_types = extract_function::to_param_types(typed_params.clone());
    let params_vec = extract_function::params_json_vec(untyped_params.clone());
    let unpacked_path_params = extract_function::unpack_path_params(&untyped_params);
    let result_type = extract_function::get_result_type(&signature.output);
    let dummy_function = extract_function::make_dummy_function(
        original_target_function,
        &function_name_string,
        untyped_params,
    );

    // todo extract any docs from passed function and put into fn wrapper

    // read current file to access imports and local functions
    let sanitized_file = extract_function::get_sanitized_file(&function);
    // todo make code below hygienic in case sanitized_file also imports from actix_web
    let main_file = quote! {
        use actix_web::{get, web, HttpResponse, Result};

        #sanitized_file
        #dummy_function
        #target_function

        #[get(#wrapper_route)]
        async fn turbolift_wrapper(path: web::Path<(#param_types,)>) -> Result<HttpResponse> {
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
                        .service(
                            turbolift_wrapper
                        )
            )
            .bind(ip_and_port)?
            .run()
            .await
        }
    };

    // copy all files in repo into cache
    let function_cache_proj_path = CACHE_PATH.join(original_target_function_name.clone());
    fs::create_dir_all(function_cache_proj_path.clone()).unwrap();
    let files_to_copy: Vec<PathBuf> = fs::read_dir(".")
        .expect("could not read dir")
        .map(|res| res.expect("could not read entry").path())
        .filter(|path| path.file_name() != CACHE_PATH.file_name())
        .filter(
            |path| path.to_str() != Some("./target"), // todo we could shorten compile time by sharing deps in ./target,
                                                      // but I didn't have the bandwidth to debug permissions errors caused
                                                      // by copying all of the compiled lib files.
        )
        .collect();
    fs_extra::copy_items(
        &files_to_copy,
        function_cache_proj_path.clone(),
        &fs_extra::dir::CopyOptions {
            overwrite: true,
            ..Default::default()
        },
    )
    .expect("error copying items to build cache");

    // edit project main file
    let target_main_file = function_cache_proj_path.join("src").join("main.rs");
    fs::write(target_main_file, main_file.to_string()).expect("error editing project main.rs");

    // modify cargo.toml (edit package info & add actix + json_serde deps)
    build_project::edit_cargo_file(
        &function_cache_proj_path.join("Cargo.toml"),
        &original_target_function_name,
    )
    .expect("error editing cargo file");

    // lint project
    build_project::lint(&function_cache_proj_path).expect("linting error");

    // check project and give errors
    build_project::check(&function_cache_proj_path).expect("error checking function");

    // // build project so that the deps are packaged, and if the worker has the same architecture,
    // // they can directly use the compiled version without having to recompile. todo commented
    // // out because the build artifacts are too large.
    // build_project::make_executable(
    //     &function_cache_proj_path,
    //     None
    // ).expect("error building function");
    // ^ todo

    // compress project source files
    let project_source_binary = {
        let tar = extract_function::make_compressed_proj_src(&function_cache_proj_path);
        let tar_file = CACHE_PATH.join(original_target_function_name.clone() + "_source.tar");
        fs::write(&tar_file, tar).expect("failure writing bin");
        TokenStream2::from_str(&format!(
            "std::include_bytes!(\"{}\")",
            tar_file
                .canonicalize()
                .expect("error canonicalizing tar file location")
                .to_str()
                .expect("failure converting file path to str")
        ))
        .expect("syntax error while embedding project tar.")
    };

    // generate API function for the microservice
    let declare_and_dispatch = quote! {
        extern crate turbolift;

        // dispatch call and process response
        async fn #original_target_function_ident(#typed_params) ->
            turbolift::DistributionResult<#result_type> {
            use std::time::Duration;
            use turbolift::DistributionPlatform;
            use turbolift::async_std::task;
            use turbolift::cached::proc_macro::cached;

            // call .declare once by memoizing the call
            #[cached(size=1)]
            fn setup() {
                #distribution_platform
                    .lock()
                    .unwrap()
                    .declare(#original_target_function_name, #project_source_binary);
            }
            setup();

            let params = #params_vec.join("/");

            let resp_string = #distribution_platform
                .lock()?
                .dispatch(
                    #original_target_function_name,
                    params.to_string()
                ).await?;
            Ok(turbolift::serde_json::from_str(&resp_string)?)
        }
    };
    declare_and_dispatch.into()
}

#[proc_macro_attribute]
pub fn with(_attr: TokenStream, _item: TokenStream) -> TokenStream {
    unimplemented!()
}
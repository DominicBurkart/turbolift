#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use std::fs;

use tar::Builder;
use quote::quote;
use turbolift::*;

#[proc_macro_attribute]
pub fn on(distribution_platform_: TokenStream, function_: TokenStream) -> TokenStream {
    let distribution_platform = TokenStream2::from(distribution_platform_);
    let function = TokenStream2::from(function_);
    // generate derived syntax
    let function_name = extract_function::get_function_name(&function);
    let typed_params = extract_function::get_typed_params(&function);
    let untyped_params = extract_function::to_untyped_params(&typed_params);
    let params_as_path = extract_function::to_path_params(&untyped_params);
    let param_types = extract_function::to_param_types(&typed_params);
    let params_vec = extract_function::params_json_vec(&param_types);
    let unpacked_path_params = extract_function::unpack_path_params(&untyped_params);
    let result_type = extract_function::get_result_type(&function);

    // todo extract any docs from passed function and put into fn wrapper

    // read current file to access imports and local functions
    let sanitized_file = extract_function::get_sanitized_file(&function);

    let wrapper_fn = quote! {
        async fn turbolift_wrapper(path: web::Path<(#param_types)>) -> Result<HttpResponse> {
            Ok(
                HttpResponse::Ok()
                    .json(#function_name(#unpacked_path_params))
            )
        }
    };

    let server = quote! {
        HttpServer::new(
            ||
                App::new()
                    .route(
                        "/" + #function_name + "/" + #params_as_path,
                        web::to(wrapper)
                    )
        )
        .bind(ip_and_port)?
        .run()
        .await
    };

    let main_file = quote! {
        use actix_web::{web, HttpResponse, Result};
        #sanitized_file

        #wrapper_fn

        #[actix_rt::main]
        async fn main() -> std::io::Result<()> {
            use actix_web::{App, HttpServer};

            let args: Vec<String> = std::env::args().collect();
            let ip_and_port = &args[1];
            #server
        }
    };

    // copy all files in repo into cache
    let function_cache_proj_path = CACHE_PATH.join(function_name);
    fs::create_dir_all(function_cache_proj_path).unwrap();
    unimplemented!();

    // modify cargo.toml (edit package info & add actix + json_serde deps)
    unimplemented!();

    // build project and give helpful compile-time errors
    build_project::make_executable(&function_cache_proj_path, None);

    // compress project source files
    let project_source_binary = extract_function::bin_vector_to_literal_tokens(
        extract_function::make_compressed_proj_src(&function_cache_proj_path)
    );

    let declare_and_dispatch = quote! {
        extern crate turbolift;
        extern crate serde_json;

        // register binary with distribution platform
        #distribution_platform.declare(#function_name, #project_source_binary);

        // dispatch call and process response
        async fn #function_name(#typed_params) -> turbolift::distributed_platform::DistributionResult<#result_type> {
            let params = #params_vec.join("/");
            let resp_str = #distribution_platform.dispatch(#function_name, params).await?;
            // ^ todo change dispatch to return a stream/iterator
            serde_json::from_str(&resp_str)
        }
    };
    declare_and_dispatch.into()
}

#[proc_macro_attribute]
pub fn with(_attr: TokenStream, item: TokenStream) -> TokenStream {
    unimplemented!()
}
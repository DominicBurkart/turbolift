use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote as q;

use turbolift_internals::extract_function;

#[cfg(feature = "distributed")]
#[proc_macro_attribute]
#[tracing::instrument]
pub fn on(distribution_platform_: TokenStream, function_: TokenStream) -> TokenStream {
    use quote::{format_ident, ToTokens};
    use std::fs;
    use std::path::PathBuf;
    use std::str::FromStr;

    use turbolift_internals::{build_project, CACHE_PATH};

    const RUN_ID_NAME: &str = "_turbolift_run_id";

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
    let mut untyped_params_with_run_id = untyped_params.clone();
    // we need to prepend a variable for the run id, since it's in the URL.
    untyped_params_with_run_id.insert(
        0,
        Box::new(syn::Pat::Ident(syn::PatIdent {
            attrs: Vec::new(),
            by_ref: None,
            mutability: None,
            ident: Ident::new(RUN_ID_NAME, Span::call_site()),
            subpat: None,
        })),
    );
    let untyped_params_tokens_with_run_id = untyped_params_with_run_id.to_token_stream();
    let untyped_params_tokens = untyped_params.to_token_stream();
    let params_as_path = extract_function::to_path_params(untyped_params.clone());
    let wrapper_route = format!(
        "{}/{{{}}}/{}",
        original_target_function_name, RUN_ID_NAME, &params_as_path
    );
    let mut param_types = extract_function::to_param_types(typed_params.clone());
    // we need to prepend a type for the run id added to the wrapper route
    param_types.insert(
        0,
        Box::new(syn::Type::Verbatim(
            str::parse::<TokenStream2>("String")
                .expect("could not parse \"String\" as a tokenstream"),
        )),
    );
    let params_vec = extract_function::params_json_vec(untyped_params.clone());
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
    let main_file = q! {
        #sanitized_file
        use turbolift::tokio_compat_02::FutureExt;

        #dummy_function
        #target_function

        async fn health_probe(_req: turbolift::actix_web::HttpRequest) -> impl turbolift::actix_web::Responder {
            turbolift::actix_web::HttpResponse::Ok()
        }

        #[turbolift::tracing::instrument]
        async fn turbolift_wrapper(turbolift::actix_web::web::Path((#untyped_params_tokens_with_run_id)): turbolift::actix_web::web::Path<(#param_types)>) -> impl turbolift::actix_web::Responder {
            turbolift::actix_web::HttpResponse::Ok()
                .json(#function_name(#untyped_params_tokens))
        }

        #[turbolift::tracing::instrument]
        fn main() {
            turbolift::actix_web::rt::System::new("main".to_string())
                .block_on(async move {
                    let args: Vec<String> = std::env::args().collect();
                    let ip_and_port = &args[1];
                    turbolift::tracing::info!("service main() started. ip_and_port parsed.");
                    turbolift::actix_web::HttpServer::new(
                        ||
                            turbolift::actix_web::App::new()
                                .route(
                                    "#wrapper_route", turbolift::actix_web::web::get().to(turbolift_wrapper)
                                )
                                .route(
                                    "/health-probe", turbolift::actix_web::web::get().to(health_probe)
                                )
                                .default_service(
                                    turbolift::actix_web::web::get()
                                        .to(
                                            |req: turbolift::actix_web::HttpRequest|
                                                turbolift::actix_web::HttpResponse::NotFound().body(
                                                    format!("endpoint not found: {}", req.uri())
                                                )
                                        )
                                )
                    )
                    .bind(ip_and_port)?
                    .run()
                    .compat()
                    .await
                }).unwrap();
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
            |path| path.to_str() != Some("./target"),
            // todo we could shorten compile time by sharing deps in ./target,
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
        PathBuf::from_str(".")
            .expect("could not find project dir")
            .canonicalize()
            .expect("could not canonicalize path to project dir")
            .as_path(),
        &function_cache_proj_path.join("Cargo.toml"),
        &original_target_function_name,
    )
    .expect("error editing cargo file");

    // lint project
    if let Err(e) = build_project::lint(&function_cache_proj_path) {
        tracing::error!(
            error = e.as_ref() as &(dyn std::error::Error + 'static),
            "ignoring linting error"
        );
    }

    // // check project and give errors
    // build_project::check(&function_cache_proj_path).expect("error checking function");

    // println!("building microservice");
    // // build project so that the deps are packaged, and if the worker has the same architecture,
    // // they can directly use the compiled version without having to recompile. todo the build artifacts are too large.
    // build_project::make_executable(&function_cache_proj_path, None)
    //     .expect("error building function");
    // // ^ todo

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
    let declare_and_dispatch = q! {
        extern crate turbolift;

        // dispatch call and process response
        #[turbolift::tracing::instrument]
        async fn #original_target_function_ident(#typed_params) ->
            turbolift::DistributionResult<#result_type> {
            use std::time::Duration;
            use turbolift::distributed_platform::DistributionPlatform;
            use turbolift::DistributionResult;
            use turbolift::tokio_compat_02::FutureExt;
            use turbolift::uuid::Uuid;

            let mut platform = #distribution_platform.lock().await;

            if !platform.has_declared(#original_target_function_name) {
                println!("declaring target function");
                platform
                    .declare(#original_target_function_name, #project_source_binary)
                    .compat()
                    .await?;
                println!("target function declared");
            }

            let params = #params_vec.join("/");
            println!("dispatching request");
            let resp_string = platform
                .dispatch(
                    #original_target_function_name,
                    params.to_string()
                )
                .compat()
                .await?;
            println!("response: {}", resp_string);
            Ok(turbolift::serde_json::from_str(&resp_string)?)
        }
    };
    declare_and_dispatch.into()
}

#[cfg(not(feature = "distributed"))]
#[proc_macro_attribute]
pub fn on(_distribution_platform: TokenStream, function_: TokenStream) -> TokenStream {
    // convert proc_macro::TokenStream to proc_macro2::TokenStream
    let function = TokenStream2::from(function_);
    let mut wrapped_original_function = extract_function::get_fn_item(function);
    let original_target_function_ident = wrapped_original_function.sig.ident.clone();
    let signature = wrapped_original_function.sig.clone();
    let typed_params = signature.inputs;
    let untyped_params = extract_function::to_untyped_params(typed_params.clone());
    let output_type = extract_function::get_result_type(&signature.output);
    wrapped_original_function.sig.ident = Ident::new("wrapped_function", Span::call_site());

    let async_function = q! {
        extern crate turbolift;

        #[turbolift::tracing::instrument]
        async fn #original_target_function_ident(#typed_params) -> turbolift::DistributionResult<#output_type> {
            #wrapped_original_function
            Ok(wrapped_function(#untyped_params))
        }
    };
    async_function.into()
}

#[proc_macro_attribute]
pub fn with(_attr: TokenStream, _item: TokenStream) -> TokenStream {
    unimplemented!()
}

extern crate proc_macro;
use proc_macro2::TokenStream;
use std::collections::{VecDeque, HashMap};
use std::path::Path;
use std::future::Future;
use std::process::Command;
use std::sync::{Arc, Mutex};

use url::Url;
use quote::quote;
use syn::{self, DeriveInput};
use reqwest::{self, Result};
use async_trait::async_trait;

use crate::distributed_platform::{DistributionPlatform, DistributionResult, ArgsString, JsonResponse};
use crate::extract_function::decompress_proj_src;
use crate::build_project::make_executable;

type AddressAndPort = Url;
type FunctionName = String;

#[derive(Default)]
pub struct LocalQueue {
    fn_name_to_address: HashMap<FunctionName, AddressAndPort>, // todo hardcoded as 127.0.0.1:8088 rn
    fn_name_to_process: HashMap<FunctionName,std::process::Child>
}

impl LocalQueue {
    pub fn new() -> LocalQueue {
        Default::default()
    }
}

#[async_trait]
impl DistributionPlatform for LocalQueue {
    /// declare a function. Runs once.
    fn declare(&mut self, function_name: &str, project_binary: &[u8]) {
        let build_dir = Path::new(".")
            .join(".turbolift")
            .join(".worker_build_cache");
        decompress_proj_src(project_binary, build_dir.as_path()).unwrap();
        let function_executable = Path::new(".").join(function_name);
        make_executable(build_dir.as_path(), Some(&function_executable)).unwrap();
        std::fs::remove_dir_all(build_dir).unwrap()
    }

    // dispatch params to a function. Runs each time the function is called.
    async fn dispatch(&mut self, function_name: &str, params: ArgsString) -> DistributionResult<JsonResponse> {
        let address_and_port = {
            if self.fn_name_to_address.contains_key(function_name) {
                // the server is already initialized.
                self
                    .fn_name_to_address
                    .get(function_name)
                    .unwrap()
                    .to_owned()
            } else {
                // we must initialize the server before sending any requests!
                let address_and_port: AddressAndPort = Url::parse("127.0.0.1:8088")?;
                let executable = Path::new(".").join(function_name);
                let server_handle = Command::new(executable)
                    .arg(&address_and_port.to_string())
                    .spawn()?;
                self
                    .fn_name_to_address
                    .insert(
                    function_name.to_string(),
                    address_and_port.clone()
                ).unwrap();
                self
                    .fn_name_to_process
                    .insert(function_name.to_string(), server_handle);
                address_and_port
            }
        };

        // request from server
        let prefixed_params = "./".to_owned() + &params;
        let query_url = address_and_port.join(&prefixed_params)?;
        let response = reqwest::get(query_url)
            .await?
            .text()
            .await?;
        Ok(response)
    }
}

impl Drop for LocalQueue {
    /// terminate all servers when program is finished
    fn drop(&mut self) {
        self
            .fn_name_to_process
            .drain()
            .for_each(
                |(_filename, mut handle)|
                    handle
                        .kill()
                        .unwrap()
            );
    }
}
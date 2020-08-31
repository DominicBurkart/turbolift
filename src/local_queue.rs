extern crate proc_macro;
use proc_macro2::TokenStream;
use std::collections::{VecDeque, HashMap};
use std::path::Path;
use std::future::Future;
use std::process::Command;
use url::Url;

use quote::quote;
use syn::{self, DeriveInput};
use tempfile;
use reqwest::{self, Result};
use async_trait::async_trait;

use crate::distributed_platform::{DistributionPlatform, DistributionResult, ArgsString, JsonResponse};
use crate::extract_function::decompress_proj_src;
use crate::build_project::make_executable;

type AddressAndPort = Url;
type FunctionName = String;

#[derive(Default)]
pub struct LocalQueue {
    queue: VecDeque<ArgsString>,
    fn_name_to_address: HashMap<FunctionName, AddressAndPort>, // todo hardcoded as 127.0.0.1:8088 rn
    fn_name_to_process: HashMap<FunctionName,std::process::Child>
}

#[async_trait]
impl DistributionPlatform for LocalQueue {
    /// declare a function
    fn declare(&mut self, function_name: &str, project_binary: Vec<u8>) {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_dir_path = temp_dir.path();
        decompress_proj_src(project_binary, temp_dir_path);
        let function_executable = Path::new(".").join(function_name);
        make_executable(temp_dir_path, Some(&function_executable));
    }

    // dispatch params to a function
    async fn dispatch(&mut self, function_name: String, params: ArgsString) -> JsonResponse {
        let address_and_port = {
            if self.fn_name_to_address.contains_key(&function_name) {
                self
                    .fn_name_to_address
                    .get(&function_name)
                    .unwrap()
                    .to_owned()
            } else {
                // we must initialize the server before sending any requests!
                let address_and_port: AddressAndPort = Url::parse("127.0.0.1:8088").unwrap();
                let executable = Path::new(".").join(function_name.clone());
                let server_handle = Command::new(executable)
                    .arg(&address_and_port.to_string())
                    .spawn()
                    .unwrap();
                self.fn_name_to_address.insert(function_name.clone(), address_and_port.clone()).unwrap();
                self.fn_name_to_process.insert(function_name, server_handle);
                address_and_port
            }
        };

        // request from server
        let prefixed_params = "./".to_owned() + &params;
        let query_url = address_and_port
            .join(&prefixed_params)
            .unwrap();
        reqwest::get(query_url)
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
            .into()
    }
}

impl Drop for LocalQueue {
    fn drop(&mut self) {
        // terminate all servers
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


#[cfg(test)]
mod tests {
    use super::*;

    #[macro_use(c)]
    extern crate cute;

    use futures::executor::block_on;
    use futures::future::try_join_all;

    #[test]
    fn it_works() {
        const LOCAL: LocalQueue = Default::default();

        #[on(LOCAL)]
        fn ident(b: bool) -> bool {
            b
        }

        let inputs = c![rand::random(), for _ in 0..10];
        let futures = {
            let mut v = Vec::new();
            for b in inputs {
                v.push(ident(b));
            }
            v
        };
        let results = block_on(try_join_all(futures)).unwrap();
        assert_eq!(
            inputs,
            results
        );
    }
}
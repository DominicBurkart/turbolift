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
    fn declare(&mut self, function_name: &str, project_binary: &[u8]) {
        unimplemented!()
    }

    async fn dispatch(&mut self, function_name: &str, params: ArgsString) -> DistributionResult<JsonResponse> {
        unimplemented!()
    }
}

impl Drop for LocalQueue {
    fn drop(&mut self) {
        unimplemented!()
    }
}
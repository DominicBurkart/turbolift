extern crate proc_macro;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use async_trait::async_trait;
use std::process::{Child, Command};
use url::Url;

use crate::build_project::make_executable;
use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};
use crate::extract_function::decompress_proj_src;
use crate::CACHE_PATH;

type AddressAndPort = Url;
type FunctionName = String;

#[derive(Default)]
pub struct LocalQueue {
    fn_name_to_address: HashMap<FunctionName, AddressAndPort>, // todo hardcoded rn
    fn_name_to_process: HashMap<FunctionName, Child>,
    fn_name_to_binary_path: HashMap<FunctionName, std::path::PathBuf>,
}

impl LocalQueue {
    pub fn new() -> LocalQueue {
        Default::default()
    }

    async fn get(query_url: Url) -> String {
        reqwest::get(query_url)
            .await
            .expect("error unwrapping local queue get response")
            .text()
            .await
            .expect("error unwrapping local queue text response")
    }
}

#[async_trait]
impl DistributionPlatform for LocalQueue {
    /// declare a function. Runs once.
    async fn declare(&mut self, function_name: &str, project_tar: &[u8]) -> DistributionResult<()> {
        let relative_build_dir = Path::new(".")
            .join(".turbolift")
            .join(".worker_build_cache");
        fs::create_dir_all(&relative_build_dir)?;
        let build_dir = relative_build_dir.canonicalize()?;
        decompress_proj_src(project_tar, &build_dir).unwrap();
        let function_executable =
            Path::new(CACHE_PATH.as_os_str()).join(function_name.to_string() + "_server");
        make_executable(&build_dir.join(function_name), Some(&function_executable))?;
        self.fn_name_to_binary_path
            .insert(function_name.to_string(), function_executable);
        //std::fs::remove_dir_all(build_dir.join(function_name)).unwrap(); todo
        Ok(())
    }

    // dispatch params to a function. Runs each time the function is called.
    async fn dispatch(
        &mut self,
        function_name: &str,
        params: ArgsString,
    ) -> DistributionResult<JsonResponse> {
        let address_and_port = {
            if self.fn_name_to_address.contains_key(function_name) {
                // the server is already initialized.
                self.fn_name_to_address
                    .get(function_name)
                    .unwrap()
                    .to_owned()
            } else {
                // we must initialize the server before sending any requests!
                let server_address_and_port_str = "127.0.0.1:8101";
                let server_url: AddressAndPort =
                    Url::parse(&("http://".to_string() + server_address_and_port_str))?;
                let executable = self.fn_name_to_binary_path.get(function_name).unwrap();
                let server_handle = Command::new(executable)
                    .arg(&server_address_and_port_str)
                    .spawn()?;
                sleep(Duration::from_secs(30));
                // ^ sleep to make sure the server is initialized before continuing
                // todo: here and with the GET request, futures hang indefinitely. To investigate.
                self.fn_name_to_address
                    .insert(function_name.to_string(), server_url.clone());
                self.fn_name_to_process
                    .insert(function_name.to_string(), server_handle);
                server_url
            }
        };

        // request from server
        let prefixed_params = "./".to_string() + function_name + "/" + &params;
        let query_url = address_and_port.join(&prefixed_params)?;

        let handle = tokio::runtime::Handle::current();
        let response = handle.block_on(Self::get(query_url));
        Ok(response)
    }

    fn has_declared(&self, fn_name: &str) -> bool {
        self.fn_name_to_binary_path.contains_key(fn_name)
    }
}

impl Drop for LocalQueue {
    /// terminate all servers when program is finished
    fn drop(&mut self) {
        self.fn_name_to_process
            .drain()
            .for_each(|(_filename, mut handle)| handle.kill().unwrap());
    }
}

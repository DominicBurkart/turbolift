extern crate proc_macro;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use std::process::{Child, Command};
use tokio_compat_02::FutureExt;
use url::Url;

use crate::build_project::make_executable;
use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};
use crate::extract_function::decompress_proj_src;
use crate::CACHE_PATH;
use uuid::Uuid;

type AddressAndPort = Url;
type FunctionName = String;

#[derive(Default, Debug)]
pub struct LocalQueue {
    fn_name_to_address: HashMap<FunctionName, AddressAndPort>, // todo hardcoded rn
    fn_name_to_process: HashMap<FunctionName, Child>,
    fn_name_to_binary_path: HashMap<FunctionName, std::path::PathBuf>,
    request_client: reqwest::Client,
}

impl LocalQueue {
    pub fn new() -> LocalQueue {
        Default::default()
    }
}

#[async_trait]
impl DistributionPlatform for LocalQueue {
    /// declare a function. Runs once.
    #[tracing::instrument(skip(project_tar))]
    async fn declare(
        &mut self,
        function_name: &str,
        run_id: Uuid,
        project_tar: &[u8],
    ) -> DistributionResult<()> {
        let relative_build_dir = Path::new(".")
            .join(".turbolift")
            .join(".worker_build_cache");
        fs::create_dir_all(&relative_build_dir)?;
        let build_dir = relative_build_dir.canonicalize()?;
        decompress_proj_src(project_tar, &build_dir).unwrap();
        let function_executable = Path::new(CACHE_PATH.as_os_str()).join(format!(
            "{}_{}_server",
            function_name.to_string(),
            run_id.as_u128()
        ));
        make_executable(&build_dir.join(function_name), Some(&function_executable))?;
        self.fn_name_to_binary_path
            .insert(function_name.to_string(), function_executable);
        //std::fs::remove_dir_all(build_dir.join(function_name)).unwrap(); todo
        Ok(())
    }

    // dispatch params to a function. Runs each time the function is called.
    #[tracing::instrument]
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
                tracing::info!("spawning");
                let server_handle = Command::new(executable)
                    .arg(&server_address_and_port_str)
                    .spawn()?;
                tracing::info!("delaying");
                tokio::time::sleep(Duration::from_secs(60)).await;
                tracing::info!("delay completed");
                // ^ sleep to make sure the server is initialized before continuing
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

        tracing::info!("sending dispatch request");
        println!("sending dispatch request to {}", query_url.as_str());
        let resp = Ok(self
            .request_client
            .get(query_url)
            .send()
            .compat()
            .await?
            .text()
            .compat()
            .await?);
        println!("received response");
        resp
    }

    #[tracing::instrument]
    fn has_declared(&self, fn_name: &str) -> bool {
        self.fn_name_to_binary_path.contains_key(fn_name)
    }
}

impl Drop for LocalQueue {
    /// terminate all servers when program is finished
    #[tracing::instrument]
    fn drop(&mut self) {
        self.fn_name_to_process
            .drain()
            .for_each(|(_filename, mut handle)| handle.kill().unwrap());
    }
}

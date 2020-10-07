use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, PostParams};
use kube::Client;
use regex::Regex;
use url::Url;

use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};

const K8S_NAMESPACE: &str = "turbolift";
type ImageTag = String;

#[derive(Default)]
pub struct K8s {
    fn_names_to_pods: HashMap<String, Pod>,
}

impl K8s {
    pub fn new() -> K8s {
        Default::default()
    }
}

#[async_trait]
impl DistributionPlatform for K8s {
    async fn declare(&mut self, function_name: &str, project_tar: &[u8]) -> DistributionResult<()> {
        // connect to cluster. tries in-cluster configuration first, then falls back to kubeconfig file.
        let client = Client::try_default().await?;
        let pods: Api<Pod> = Api::namespaced(client, K8S_NAMESPACE);

        // generate image & host it on a local repo
        let repo_url = setup_repo(function_name, project_tar)?;
        let local_tag = make_image(function_name, project_tar)?;
        let tag_in_repo = add_image_to_repo(local_tag)?;
        let image_url = repo_url.join(&tag_in_repo)?;

        // make pod
        let pod_name = function_name;
        let container_name = function_name.to_string() + "-container";
        let pod = serde_json::from_value(serde_json::json!({
            "apiVersion": "v1",
            "kind": "Pod",
            "metadata": {
                "name": pod_name
            },
            "spec": {
                "containers": [
                    {
                        "name": container_name,
                        "image": image_url.as_str(),
                    },
                ],
            }
        }))?;
        self.fn_names_to_pods.insert(
            function_name.to_string(),
            pods.create(&PostParams::default(), &pod).await?,
        );
        // todo we should make sure that the pod is accepted, and should make sure it didn't error
        Ok(())
    }

    async fn dispatch(
        &mut self,
        _function_name: &str,
        _params: ArgsString,
    ) -> DistributionResult<JsonResponse> {
        unimplemented!()
    }

    fn has_declared(&self, fn_name: &str) -> bool {
        self.fn_names_to_pods.contains_key(fn_name)
    }
}

lazy_static! {
    static ref PORT_RE: Regex = Regex::new(r"0\.0\.0\.0:(\d+)->").unwrap();
}

fn setup_repo(_function_name: &str, _project_tar: &[u8]) -> anyhow::Result<Url> {
    // check if registry is already running locally
    let ps_output = Command::new("docker")
        .args("ps -f name=registry".split(' '))
        .output()?;
    if !ps_output.status.success() {
        return Err(anyhow::anyhow!(
            "docker ps failed. Is the docker daemon running?"
        ));
    }
    let utf8 = String::from_utf8(ps_output.stdout)?;
    let num_lines = utf8.as_str().lines().count();
    let port = if num_lines == 1 {
        // registry not running. Start the registry.
        let status = Command::new("docker")
            .args("run -d -p 5000:5000 --restart=always --name registry registry:2".split(' '))
            .status()?; // todo choose an open port instead of just hoping 5000 is open
        if !status.success() {
            return Err(anyhow::anyhow!("repo setup failed"));
        }
        "5000"
    } else {
        // registry is running. Return for already-running registry.
        PORT_RE.captures(&utf8).unwrap().get(1).unwrap().as_str()
    };

    // return local ip + the registry port
    let interfaces = get_if_addrs::get_if_addrs()?;
    for interface in &interfaces {
        if interface.name == "en0" {
            // todo support other network interfaces
            let ip = interface.addr.ip();
            let ip_and_port = "http://".to_string() + &ip.to_string() + ":" + port;
            return Ok(Url::from_str(&ip_and_port)?);
        }
    }
    Err(anyhow::anyhow!(
        "no en0 interface found. interfaces: {:?}",
        interfaces
    ))
}

fn make_image(function_name: &str, project_tar: &[u8]) -> anyhow::Result<ImageTag> {
    println!("making image");
    // set up directory and dockerfile
    let build_dir = std::path::PathBuf::from(format!("{}_k8s_temp_dir", function_name));
    std::fs::create_dir_all(&build_dir)?;
    let build_dir_canonical = build_dir.canonicalize()?;
    let dockerfile_path = build_dir_canonical.join("Dockerfile");
    let tar_file_name = "source.tar";
    let tar_path = build_dir_canonical.join(tar_file_name);
    let docker_file = format!(
        "FROM rustlang/rust:nightly
COPY {} {}
RUN cat {} | tar xvf -
WORKDIR {}
ENTRYPOINT [\"cargo\", \"run\"]",
        tar_file_name, tar_file_name, tar_file_name, function_name
    );
    std::fs::write(&dockerfile_path, docker_file)?;
    std::fs::write(&tar_path, project_tar)?;

    let result = (|| {
        // build image
        let build_cmd = format!(
            "build -t {} {}",
            function_name,
            build_dir_canonical.to_string_lossy()
        );
        let status = Command::new("docker")
            .args(build_cmd.as_str().split(' '))
            .status()?;

        // make sure that build completed successfully
        if !status.success() {
            return Err(anyhow::anyhow!("docker image build failure"));
        }
        Ok(function_name.to_string())
    })();
    // always remove the build directory, even on build error
    std::fs::remove_dir_all(build_dir_canonical)?;
    result
}

fn add_image_to_repo(_local_tag: ImageTag) -> DistributionResult<ImageTag> {
    unimplemented!()
}

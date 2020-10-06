use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

use async_trait::async_trait;
use base64::encode;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, PostParams};
use kube::Client;
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

fn setup_repo(_function_name: &str, _project_tar: &[u8]) -> anyhow::Result<Url> {
    let status = Command::new("docker")
        .args("run -d -p 5000:5000 --restart=always --name registry registry:2".split(' '))
        .status()?; // todo choose an open port
    if !status.success() {
        return Err(anyhow::anyhow!("repo setup failed"));
    }
    let interfaces = get_if_addrs::get_if_addrs()?;
    for interface in interfaces {
        if interface.name == "en0" {
            // todo support other network interfaces
            let ip = interface.addr.ip();
            return Ok(Url::from_str(&(ip.to_string() + ":5000"))?);
        }
    }
    Err(anyhow::anyhow!("no en0 interface found"))
}

fn make_image(function_name: &str, project_tar: &[u8]) -> anyhow::Result<ImageTag> {
    let tar_base64 = encode(project_tar);
    let mut docker_file = format!(
        "\
FROM rustlang/rust:nightly
RUN  apt-get update \
  && apt-get install -y coreutils \
  && rm -rf /var/lib/apt/lists/*
base64 --decode {} > f.tar
tar xvf f.tar
",
        tar_base64
    );
    docker_file.insert(0, '\'');
    docker_file.push('\'');
    let tag_flag = "-t ".to_string() + function_name;
    let status = Command::new("docker")
        .args(&["build", &tag_flag, "-", &docker_file])
        .status()?;
    if status.success() {
        return Err(anyhow::anyhow!("docker image build failure"));
    }
    Ok(function_name.to_string())
}

fn add_image_to_repo(_local_tag: ImageTag) -> DistributionResult<ImageTag> {
    unimplemented!()
}

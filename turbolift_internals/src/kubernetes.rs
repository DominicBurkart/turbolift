use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use kube::api::{Api, Meta, ListParams, PostParams, WatchEvent};
use kube::Client;
use k8s_openapi::api::core::v1::Pod;
use cached::proc_macro::cached;
use url::Url;

use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};

const K8S_NAMESPACE: &str = "turbolift";
type ImageTag = String;

pub type K8sConfig = kube::config::Config;

pub struct K8s {
    config: K8sConfig,
    pods: Vec<Pod>,
}

#[async_trait]
impl DistributionPlatform for K8s {
    fn declare(&mut self, function_name: &str, project_tar: &[u8]) {
        // connect to cluster. tries in-cluster configuration first, then falls back to kubeconfig file.
        let client = Client::try_default().await?;
        let pods: Api<Pod> = Api::namespaced(client, K8S_NAMESPACE);

        // generate image & host it on a local repo
        let repo_url = setup_repo().expect("error initializing network repository");
        let local_tag = make_image(function_name, project_tar).expect("error making image");
        let tag_in_repo = add_image_to_repo(local_tag).expect("error adding image to repo");
        let image_url = repo_url.join(&tag_in_repo).expect("url parse error");

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
        self.pods.push(pods.create(&PostParams::default(), &pod).await?);
        // todo do we need to monitor the pod in any way??
    }

    async fn dispatch(&mut self, function_name: &str, params: ArgsString) -> DistributionResult<JsonResponse> {
        unimplemented!()
    }
}

#[cached(size=1)]
fn setup_repo() -> DistributionResult<Url> {
    unimplemented!()
}

fn make_image(function_name: &str, project_tar: &[u8]) -> DistributionResult<ImageTag> {
    unimplemented!()
}

fn add_image_to_repo(local_tag: ImageTag) -> DistributionResult<ImageTag> {
    unimplemented!()
}
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, PostParams};
use kube::Client;
use std::collections::HashMap;
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

fn setup_repo(_function_name: &str, _project_tar: &[u8]) -> DistributionResult<Url> {
    unimplemented!()
}

fn make_image(_function_name: &str, _project_tar: &[u8]) -> DistributionResult<ImageTag> {
    unimplemented!()
}

fn add_image_to_repo(_local_tag: ImageTag) -> DistributionResult<ImageTag> {
    unimplemented!()
}

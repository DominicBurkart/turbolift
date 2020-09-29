use async_trait::async_trait;
use cached::proc_macro::cached;
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, PostParams};
use kube::Client;
use url::Url;

use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};

const K8S_NAMESPACE: &str = "turbolift";
type ImageTag = String;

pub struct K8s {
    pods: Vec<Pod>,
}

#[async_trait]
impl DistributionPlatform for K8s {
    async fn declare(&mut self, function_name: &str, project_tar: &[u8]) -> DistributionResult<()> {
        // connect to cluster. tries in-cluster configuration first, then falls back to kubeconfig file.
        let client = Client::try_default().await?;
        let pods: Api<Pod> = Api::namespaced(client, K8S_NAMESPACE);

        // generate image & host it on a local repo
        let repo_url = setup_repo();
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
        self.pods
            .push(pods.create(&PostParams::default(), &pod).await?);
        // todo do we need to monitor the pod in any way??
        Ok(())
    }

    async fn dispatch(
        &mut self,
        _function_name: &str,
        _params: ArgsString,
    ) -> DistributionResult<JsonResponse> {
        unimplemented!()
    }
}

#[cached(size = 1)]
fn setup_repo() -> Url {
    unimplemented!()
}

fn make_image(_function_name: &str, _project_tar: &[u8]) -> DistributionResult<ImageTag> {
    unimplemented!()
}

fn add_image_to_repo(_local_tag: ImageTag) -> DistributionResult<ImageTag> {
    unimplemented!()
}

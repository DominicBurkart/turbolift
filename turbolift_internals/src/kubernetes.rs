use std::collections::HashMap;
use std::process::Command;
use std::str::FromStr;

use async_trait::async_trait;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Service;
use kube::api::{Api, PostParams};
use kube::Client;
use regex::Regex;
use syn::export::Formatter;
use tokio_compat_02::FutureExt;
use url::Url;

use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};
use crate::utils::RELEASE_FLAG;

const TURBOLIFT_K8S_NAMESPACE: &str = "default";
const LOCAL_REGISTRY_URL: &str = "http://localhost:32000";
type ImageTag = String;

/// `K8s` is the interface for turning rust functions into autoscaling microservices
/// using turbolift. It requires docker and kubernetes / kubectl to already be setup on the
/// device at runtime.
///
/// Access to the kubernetes cluster must be inferrable from the env variables at runtime.
pub struct K8s {
    max_scale_n: u32,
    fn_names_to_services: HashMap<String, Url>,
    request_client: reqwest::Client,
}

impl K8s {
    /// returns a K8s object that does not perform autoscaling.
    pub fn new() -> K8s {
        K8s::with_max_replicas(1)
    }

    /// returns a K8s object. If max is equal to 1, then autoscaling
    /// is not enabled. Otherwise, autoscale is automatically activated
    /// with cluster defaults and a max number of replicas *per distributed
    /// function* of `max`. Panics if `max` < 1.
    pub fn with_max_replicas(max: u32) -> K8s {
        if max < 1 {
            panic!("max < 1 while instantiating k8s (value: {})", max)
        }
        K8s {
            max_scale_n: max,
            fn_names_to_services: HashMap::new(),
            request_client: reqwest::Client::new(),
        }
    }
}

impl Default for K8s {
    fn default() -> Self {
        K8s::new()
    }
}

pub struct AutoscaleError {}
impl std::error::Error for AutoscaleError {}
impl std::fmt::Debug for AutoscaleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "error while applying autoscale")
    }
}
impl std::fmt::Display for AutoscaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "error while applying autoscale")
    }
}

fn function_to_service_name(function_name: &str) -> String {
    function_name.to_string().replace("_", "-") + "-service"
}

fn function_to_deployment_name(function_name: &str) -> String {
    function_name.to_string().replace("_", "-") + "-deployment"
}

fn function_to_app_name(function_name: &str) -> String {
    function_name.to_string().replace("_", "-")
}

fn function_to_container_name(function_name: &str) -> String {
    function_name.to_string().replace("_", "-") + "-container"
}

#[async_trait]
impl DistributionPlatform for K8s {
    async fn declare(&mut self, function_name: &str, project_tar: &[u8]) -> DistributionResult<()> {
        // connect to cluster. tries in-cluster configuration first, then falls back to kubeconfig file.
        let deployment_client = Client::try_default().compat().await?;
        let deployments: Api<Deployment> =
            Api::namespaced(deployment_client, TURBOLIFT_K8S_NAMESPACE);
        let service_client = Client::try_default().compat().await?;
        let services: Api<Service> = Api::namespaced(service_client, TURBOLIFT_K8S_NAMESPACE);

        // generate image & host it on a local registry
        let registry_url = Url::parse(LOCAL_REGISTRY_URL)?;
        let tag_in_reg = make_image(function_name, project_tar, &registry_url)?;
        let image_url = registry_url.join(&tag_in_reg)?.as_str().to_string();

        println!("wooo");
        let deployment_name = function_to_deployment_name(function_name);
        let service_name = function_to_service_name(function_name);
        println!("got service_name");
        let app_name = function_to_app_name(function_name);
        let container_name = function_to_container_name(function_name);
        println!("... app_name is fine...");

        // make deployment
        let deployment_json = serde_json::json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {
                "name": deployment_name,
                "labels": {
                    "app": app_name
                }
            },
            "spec": {
                "replicas": 1,
                "selector": {
                    "matchLabels": {
                        "app": app_name
                    }
                },
                "template": {
                    "metadata": {
                        "labels": {
                            "app": app_name
                        }
                    },
                    "spec": {
                        "containers": [
                            {
                                "name": container_name,
                                "image": image_url,
                                "ports": [
                                    {
                                        "containerPort": 5000
                                    }
                                ]
                            },
                        ]
                    }
                }
            }
        });
        println!("deployment_json generated, {:?}", deployment_json);
        let deployment = serde_json::from_value(deployment_json)?;
        println!("made deployment: {:?}", deployment);
        deployments
            .create(&PostParams::default(), &deployment)
            .compat()
            .await?;
        println!("created deployment");

        // make service pointing to deployment
        let service = serde_json::from_value(serde_json::json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": service_name
            },
            "spec": {
                "type": "NodePort",
                "selector": {
                    "app": deployment_name
                },
                "ports": [
                    {
                        "protocol": "TCP",
                        "port": 5000
                    }
                ]
            }
        }))?;
        println!("made service");
        let service = services
            .create(&PostParams::default(), &service)
            .compat()
            .await?;
        println!("created service");
        let node_port = service
            .spec
            .expect("no specification found for service")
            .ports
            .expect("no ports found for service")
            .iter()
            .filter_map(|port| port.node_port)
            .next()
            .expect("no node port assigned to service");
        let service_ip = format!("http://localhost:{}", node_port);
        println!("service_ip {}", service_ip);

        // todo make sure that the pod and service were correctly started before returning

        if self.max_scale_n > 1 {
            // set autoscale
            let scale_args = format!(
                "autoscale deployment {} --max={}",
                deployment_name, self.max_scale_n
            );
            let scale_status = Command::new("kubectl")
                .args(scale_args.as_str().split(' '))
                .status()?;
            if !scale_status.success() {
                return Err(Box::new(AutoscaleError {}));
                // ^ todo attach error context from child
            }
        }
        self.fn_names_to_services
            .insert(function_name.to_string(), Url::from_str(&service_ip)?);
        // todo handle deleting the relevant service and deployment for each distributed function.
        Ok(())
    }

    async fn dispatch(
        &mut self,
        function_name: &str,
        params: ArgsString,
    ) -> DistributionResult<JsonResponse> {
        // request from server
        let service_base_url = self.fn_names_to_services.get(function_name).unwrap();
        let args = "./".to_string() + function_name + "/" + &params;
        let query_url = service_base_url.join(&args)?;
        println!("sending dispatch request to {:?}", query_url);
        let resp = Ok(self
            .request_client
            .get(query_url)
            .send()
            .compat()
            .await?
            .text()
            .compat()
            .await?);
        println!("dispatch returning: {:?}", resp);
        resp
    }

    fn has_declared(&self, fn_name: &str) -> bool {
        self.fn_names_to_services.contains_key(fn_name)
    }
}

lazy_static! {
    static ref PORT_RE: Regex = Regex::new(r"0\.0\.0\.0:(\d+)->").unwrap();
}

fn make_image(
    function_name: &str,
    project_tar: &[u8],
    registry_url: &Url,
) -> anyhow::Result<ImageTag> {
    println!("making image");
    // set up directory and dockerfile
    let build_dir = std::path::PathBuf::from(format!("{}_k8s_temp_dir", function_name));
    std::fs::create_dir_all(&build_dir)?;
    let build_dir_canonical = build_dir.canonicalize()?;
    let dockerfile_path = build_dir_canonical.join("Dockerfile");
    let tar_file_name = "source.tar";
    let tar_path = build_dir_canonical.join(tar_file_name);
    let docker_file = format!(
        "FROM ubuntu:latest
# set timezone (otherwise tzinfo stops dep installation with prompt for time zone)
ENV TZ=Etc/UTC
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

# install curl and rust deps
RUN apt-get update && apt-get install -y curl gcc libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

# install rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly-2020-09-28
ENV PATH=/root/.cargo/bin:$PATH

# copy tar file
COPY {} {}

# unpack tar
RUN cat {} | tar xvf -

# enter into unpacked source directory
WORKDIR {}

# build and run release
RUN RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo build {}
CMD RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run {} localhost:5000",
        tar_file_name,
        tar_file_name,
        tar_file_name,
        function_name,
        RELEASE_FLAG,
        RELEASE_FLAG
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
        let build_status = Command::new("docker")
            .args(build_cmd.as_str().split(' '))
            .status()?;

        // make sure that build completed successfully
        if !build_status.success() {
            return Err(anyhow::anyhow!("docker image build failure"));
        }

        let image_tag = format!(
            "localhost:{}/{}",
            registry_url.port().unwrap(),
            function_name
        );

        println!("image tag: {}", image_tag);

        // tag image
        let tag_args = format!("image tag {} {}", function_name, image_tag);
        let tag_result = Command::new("docker")
            .args(tag_args.as_str().split(' '))
            .status()?;
        if !tag_result.success() {
            return Err(anyhow::anyhow!("docker image tag failure"));
        }

        println!("image tag worked: {}", tag_args);

        // push image to local repo
        let push_status = Command::new("docker")
            .arg("push")
            .arg(image_tag.clone())
            .status()?;
        println!("docker push command did not explode");
        if !push_status.success() {
            return Err(anyhow::anyhow!("docker image push failure"));
        }

        Ok(image_tag)
    })();
    println!("removing build dir");
    // always remove the build directory, even on build error
    std::fs::remove_dir_all(build_dir_canonical)?;
    println!("returning res");
    result
}

impl Drop for K8s {
    fn drop(&mut self) {
        // delete the associated services and deployments from the functions we distributed
        // let rt = tokio::runtime::Runtime::new().unwrap();
        // rt.block_on(async {
        //     let deployment_client = Client::try_default().compat().await.unwrap();
        //     let deployments: Api<Deployment> =
        //         Api::namespaced(deployment_client, TURBOLIFT_K8S_NAMESPACE);
        //     let service_client = Client::try_default().compat().await.unwrap();
        //     let services: Api<Service> = Api::namespaced(service_client, TURBOLIFT_K8S_NAMESPACE);
        //
        //     let distributed_functions = self.fn_names_to_services.keys();
        //     for function in distributed_functions {
        //         let service = function_to_service_name(function);
        //         services
        //             .delete(&service, &Default::default())
        //             .compat()
        //             .await
        //             .unwrap();
        //         let deployment = function_to_deployment_name(function);
        //         deployments
        //             .delete(&deployment, &Default::default())
        //             .compat()
        //             .await
        //             .unwrap();
        //     }
        // });
        //
        // // delete the local registry
        // let registry_deletion_status = Command::new("docker")
        //     .arg("rmi")
        //     .arg("$(docker images |grep 'turbolift-registry')")
        //     .status()
        //     .unwrap();
        // if !registry_deletion_status.success() {
        //     eprintln!(
        //         "could not delete turblift registry docker image. error code: {}",
        //         registry_deletion_status.code().unwrap()
        //     );
        // }
    }
}

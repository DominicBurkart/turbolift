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
use crate::utils::get_open_socket;

const TURBOLIFT_K8S_NAMESPACE: &str = "default";
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
    function_name.to_string() + "-service"
}

fn function_to_deployment_name(function_name: &str) -> String {
    function_name.to_string() + "-deployment"
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
        let registry_url = setup_registry(function_name, project_tar)?;
        let tag_in_reg = make_image(function_name, project_tar, &registry_url)?;
        let image_url = registry_url.join(&tag_in_reg)?.as_str().to_string();

        // make deployment
        println!("wooo");
        let deployment_name = function_to_deployment_name(function_name);
        let service_name = function_to_service_name(function_name);
        println!("got service_name");
        let app_name = function_name.to_string();
        println!("... app_name is fine...");
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
                                "name": tag_in_reg,
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
                "selector": {
                    "app": deployment_name
                },
                "ports": {
                    "protocol": "HTTP",
                    "port": 5000,
                    "targetPort": 5000
                }
            }
        }))?;
        println!("made service");
        let service = services
            .create(&PostParams::default(), &service)
            .compat()
            .await?;
        println!("created service");
        let service_ip = format!(
            "http://{}:5000",
            service
                .spec
                .expect("no specification found for service")
                .cluster_ip
                .expect("no cluster ip found for service")
        );
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

fn setup_registry(_function_name: &str, _project_tar: &[u8]) -> anyhow::Result<Url> {
    // check if registry is already running locally
    let ps_output = Command::new("docker")
        .args("ps -f name=turbolift-registry".split(' '))
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
        let port = get_open_socket()?.local_addr()?.port().to_string(); // todo hack
        let args_str = format!(
            "run -d -p {}:5000 --restart=always --name turbolift-registry registry:2",
            port
        );
        let status = Command::new("docker")
            .args(args_str.as_str().split(' '))
            .status()?;
        if !status.success() {
            return Err(anyhow::anyhow!("registry setup failed"));
        }
        port
    } else {
        // turbolift-registry is running. Return for already-running registry.
        PORT_RE
            .captures(&utf8)
            .unwrap()
            .get(1)
            .unwrap()
            .as_str()
            .to_string()
    };

    // return local ip + the registry port
    let interfaces = get_if_addrs::get_if_addrs()?;
    for interface in &interfaces {
        if (interface.name == "en0") || (interface.name == "eth0") {
            // todo support other network interfaces and figure out a better way to choose the interface
            let ip = interface.addr.ip();
            let ip_and_port = "http://".to_string() + &ip.to_string() + ":" + &port;
            return Ok(Url::from_str(&ip_and_port)?);
        }
    }
    Err(anyhow::anyhow!(
        "no en0/eth0 interface found. interfaces: {:?}",
        interfaces
    ))
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
        "FROM rustlang/rust:nightly
COPY {} {}
RUN cat {} | tar xvf -
WORKDIR {}
RUN RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo build --release
CMD RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run --release 127.0.0.1:5000",
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
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let deployment_client = Client::try_default().compat().await.unwrap();
            let deployments: Api<Deployment> =
                Api::namespaced(deployment_client, TURBOLIFT_K8S_NAMESPACE);
            let service_client = Client::try_default().compat().await.unwrap();
            let services: Api<Service> = Api::namespaced(service_client, TURBOLIFT_K8S_NAMESPACE);

            let distributed_functions = self.fn_names_to_services.keys();
            for function in distributed_functions {
                let service = function_to_service_name(function);
                services
                    .delete(&service, &Default::default())
                    .compat()
                    .await
                    .unwrap();
                let deployment = function_to_deployment_name(function);
                deployments
                    .delete(&deployment, &Default::default())
                    .compat()
                    .await
                    .unwrap();
            }
        });

        // delete the local registry
        let registry_deletion_status = Command::new("docker")
            .arg("rmi")
            .arg("$(docker images |grep 'turbolift-registry')")
            .status()
            .unwrap();
        if !registry_deletion_status.success() {
            eprintln!(
                "could not delete turblift registry docker image. error code: {}",
                registry_deletion_status.code().unwrap()
            );
        }
    }
}

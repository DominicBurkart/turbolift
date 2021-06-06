use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::str::FromStr;

use async_trait::async_trait;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Service;
use kube::api::{Api, PostParams};
use kube::Client;
use regex::Regex;
use tokio::time::{sleep, Duration};
use tokio_compat_02::FutureExt;
use url::Url;

use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};
use crate::utils::{DEBUG_FLAG, RELEASE_FLAG};
use crate::CACHE_PATH;
use std::io::Write;
use uuid::Uuid;

const TURBOLIFT_K8S_NAMESPACE: &str = "default";
type ImageTag = String;

pub const CONTAINER_PORT: i32 = 5678;
pub const SERVICE_PORT: i32 = 5678;
pub const EXTERNAL_PORT: i32 = 80;
pub const TARGET_ARCHITECTURE: Option<&str> = None;
// ^ todo: we want the user to be able to specify something like `Some("x86_64-unknown-linux-musl")`
//   during config, but right now that doesn't work because we are relying on super unstable
//   span features to extract functions into services. When we can enable statically linked
//   targets, we can use the multi-stage build path and significantly reduce the size.

/// `K8s` is the interface for turning rust functions into autoscaling microservices
/// using turbolift. It requires docker and kubernetes / kubectl to already be setup on the
/// device at runtime.
///
/// Access to the kubernetes cluster must be inferrable from the env variables at runtime.
#[derive(Debug)]
pub struct K8s {
    max_scale_n: u32,
    fn_names_to_services: HashMap<String, Url>,
    request_client: reqwest::Client,
}

impl K8s {
    /// returns a K8s object that does not perform autoscaling.
    #[tracing::instrument]
    pub fn new() -> K8s {
        K8s::with_max_replicas(1)
    }

    /// returns a K8s object. If max is equal to 1, then autoscaling
    /// is not enabled. Otherwise, autoscale is automatically activated
    /// with cluster defaults and a max number of replicas *per distributed
    /// function* of `max`. Panics if `max` < 1.
    #[tracing::instrument]
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

fn sanitize_function_name(function_name: &str) -> String {
    function_name.to_string().replace("_", "-")
}

#[async_trait]
impl DistributionPlatform for K8s {
    #[tracing::instrument(skip(project_tar))]
    async fn declare(
        &mut self,
        function_name: &str,
        run_id: Uuid,
        project_tar: &[u8],
    ) -> DistributionResult<()> {
        // connect to cluster. tries in-cluster configuration first, then falls back to kubeconfig file.
        let deployment_client = Client::try_default().compat().await?;
        let deployments: Api<Deployment> =
            Api::namespaced(deployment_client, TURBOLIFT_K8S_NAMESPACE);
        let service_client = Client::try_default().compat().await?;
        let services: Api<Service> = Api::namespaced(service_client, TURBOLIFT_K8S_NAMESPACE);

        // generate image & push
        let app_name = format!(
            "{}-{}",
            sanitize_function_name(function_name),
            run_id.as_u128()
        );
        let container_name = format!("{}-app", app_name);
        let deployment_name = format!("{}-deployment", app_name);
        let service_name = format!("{}-service", app_name);
        let ingress_name = format!("{}-ingress", app_name);
        let tag_in_reg = make_image(&app_name, function_name, project_tar)?;

        println!("image made. making deployment and service names.");
        println!("made service_name");
        println!("made app_name and container_name");

        // make deployment
        let deployment_json = serde_json::json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {
                "name": deployment_name,
            },
            "spec": {
                "selector": {
                    "matchLabels": {
                        "app": app_name
                    }
                },
                "replicas": 1,
                "template": {
                    "metadata": {
                     "name": format!("{}-app", app_name),
                     "labels": {
                       "app": app_name
                     }
                    },
                    "spec": {
                        "containers": [
                            {
                                "name": container_name,
                                "image": tag_in_reg
                            }
                         ]
                    }
                }
            }
        });
        println!("deployment_json generated");
        let deployment = serde_json::from_value(deployment_json)?;
        println!("deployment generated");
        deployments
            .create(&PostParams::default(), &deployment)
            .compat()
            .await?;
        println!("created deployment");

        // make service pointing to deployment
        let service_json = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": service_name,
            },
            "spec": {
                "selector": {
                    "app": app_name
                },
                "ports": [{
                    "port": SERVICE_PORT,
                    "targetPort": CONTAINER_PORT,
                }]
            }
        });
        let service = serde_json::from_value(service_json)?;
        println!("deployment generated");
        services
            .create(&PostParams::default(), &service)
            .compat()
            .await?;
        println!("created service");

        // make ingress pointing to service
        let ingress = serde_json::json!({
            "apiVersion": "networking.k8s.io/v1",
            "kind": "Ingress",
            "metadata": {
                "name": ingress_name
            },
            "spec": {
                "rules": [
                    {
                        "http": {
                            "paths": [
                                {
                                    "path": format!("/{}", app_name),
                                    "pathType": "Prefix",
                                    "backend": {
                                        "service" : {
                                            "name": app_name,
                                            "port": {
                                                "number": SERVICE_PORT
                                            }
                                        }
                                    }
                                }
                            ]
                        }
                    }
                ]
            }
        });

        let mut apply_ingress_child = Command::new("kubectl")
            .args("apply -f -".split(' '))
            .stdin(Stdio::piped())
            .spawn()?;
        apply_ingress_child
            .stdin
            .as_mut()
            .expect("not able to write to ingress apply stdin")
            .write_all(ingress.to_string().as_bytes())?;
        if !apply_ingress_child.wait()?.success() {
            panic!(
                "failed to apply ingress: {}\nis ingress enabled on this cluster?",
                ingress.to_string()
            )
        }

        let node_ip = {
            // let stdout = Command::new("kubectl")
            // .args("get nodes --selector=kubernetes.io/role!=master -o jsonpath={.items[*].status.addresses[?\\(@.type==\\\"InternalIP\\\"\\)].address}".split(' '))
            // .output()
            // .expect("error finding node ip")
            // .stdout;
            // String::from_utf8(stdout).expect("could not parse local node ip")
            "localhost".to_string()
        };
        println!("found node ip: {}", node_ip.as_str());

        // let node_port: i32 = 5000;
        // let node_port = service
        //     .spec
        //     .expect("no specification found for service")
        //     .ports
        //     .expect("no ports found for service")
        //     .iter()
        //     .filter_map(|port| port.node_port)
        //     .next()
        //     .expect("no node port assigned to service");
        // let service_ip = format!("http://{}", node_ip, node_port);
        let service_ip = format!("http://localhost:{}/{}/", EXTERNAL_PORT, app_name);
        println!("generated service_ip: {}", service_ip.as_str());

        // todo make sure that the pod and service were correctly started before returning

        // if self.max_scale_n > 1 {
        //     // set autoscale
        //     let scale_args = format!(
        //         "autoscale deployment {} --max={}",
        //         deployment_name, self.max_scale_n
        //     );
        //     let scale_status = Command::new("kubectl")
        //         .args(scale_args.as_str().split(' '))
        //         .status()?;
        //
        //     if !scale_status.success() {
        //         return Err(anyhow::anyhow!(
        //             "autoscale error: error code: {:?}",
        //             scale_status.code()
        //         )
        //         .into());
        //         // ^ todo attach error context from child
        //     }
        // }

        sleep(Duration::from_secs(600)).await;
        // todo implement the check on whether the service is running / pod failed

        self.fn_names_to_services
            .insert(function_name.to_string(), Url::from_str(&service_ip)?);
        // todo handle deleting the relevant service and deployment for each distributed function.

        println!("returning from declare");
        Ok(())
    }

    #[tracing::instrument]
    async fn dispatch(
        &mut self,
        function_name: &str,
        params: ArgsString,
    ) -> DistributionResult<JsonResponse> {
        // request from server
        let service_base_url = self.fn_names_to_services.get(function_name).unwrap();
        let args = format!("./{}", params);
        let query_url = service_base_url.join(&args)?;
        tracing::info!(url = query_url.as_str(), "sending dispatch request");
        println!("sending dispatch request");
        Ok(self
            .request_client
            .get(query_url)
            .send()
            .compat()
            .await?
            .text()
            .compat()
            .await?)
    }

    #[tracing::instrument]
    fn has_declared(&self, fn_name: &str) -> bool {
        println!("in has_declared");
        self.fn_names_to_services.contains_key(fn_name)
    }
}

lazy_static! {
    static ref PORT_RE: Regex = Regex::new(r"0\.0\.0\.0:(\d+)->").unwrap();
}

#[tracing::instrument(skip(project_tar))]
fn make_image(app_name: &str, function_name: &str, project_tar: &[u8]) -> anyhow::Result<ImageTag> {
    // todo: we should add some random stuff to the function_name to avoid collisions and figure
    // out when to overwrite vs not.

    tracing::info!("making image");
    // set up directory and dockerfile
    let build_dir = CACHE_PATH.join(format!("{}_k8s_temp_dir", app_name).as_str());
    std::fs::create_dir_all(&build_dir)?;
    let build_dir_canonical = build_dir.canonicalize()?;
    let dockerfile_path = build_dir_canonical.join("Dockerfile");
    let tar_file_name = "source.tar";
    let tar_path = build_dir_canonical.join(tar_file_name);
    let docker_file = format!(
        "FROM ubuntu:latest as builder
# set timezone (otherwise tzinfo stops dep installation with prompt for time zone)
ENV TZ=Etc/UTC
RUN ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

# install curl and rust deps
RUN apt-get update && apt-get install -y curl gcc libssl-dev pkg-config && rm -rf /var/lib/apt/lists/*

# install rustup
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly
ENV PATH=/root/.cargo/bin:$PATH

# copy tar file
COPY {tar_file_name} {tar_file_name}

# unpack tar
RUN cat {tar_file_name} | tar xvf -

# enter into unpacked source directory
WORKDIR {function_name}

# build and run according to compilation scheme
ENV RUSTFLAGS='--cfg procmacro2_semver_exempt'
{compilation_scheme}",
        function_name=function_name,
        tar_file_name=tar_file_name,
        compilation_scheme={
            if let Some(architecture) = TARGET_ARCHITECTURE {
                format!("# install the project binary with the given architecture.
RUN rustup target add {architecture}
RUN cargo install{debug_flag} --target {architecture} --path .

# copy the binary from the builder, leaving the build environment.
FROM scratch
COPY --from=builder /usr/local/cargo/bin/{function_name} .
CMD [\"./{function_name}\", \"0.0.0.0:{container_port}\"]",
                 architecture=architecture,
                 debug_flag=DEBUG_FLAG,
                 function_name=function_name,
                 container_port=CONTAINER_PORT
                )
            } else {
                format!(
                    "CMD cargo run{release_flag} -- 0.0.0.0:{container_port}",
                    release_flag=RELEASE_FLAG,
                    container_port=CONTAINER_PORT
                )
            }
        }
    );
    std::fs::write(&dockerfile_path, docker_file)?;
    std::fs::write(&tar_path, project_tar)?;
    let unique_tag = format!("{}:turbolift", app_name);

    let result = (|| {
        // build image
        let build_cmd = format!(
            "build -t {} {}",
            unique_tag,
            build_dir_canonical.to_string_lossy()
        );
        let build_status = Command::new("docker")
            .args(build_cmd.as_str().split(' '))
            .status()?;

        // make sure that build completed successfully
        if !build_status.success() {
            return Err(anyhow::anyhow!("docker image build failure"));
        }

        // let image_tag = format!(
        //     "localhost:{}/{}",
        //     registry_url.port().unwrap(),
        //     function_name
        // );

        println!("made image tag");

        // tag image
        // let tag_args = format!("image tag {} {}", function_name, function_name);
        // let tag_result = Command::new("docker")
        //     .args(tag_args.as_str().split(' '))
        //     .status()?;
        // if !tag_result.success() {
        //     return Err(anyhow::anyhow!("docker image tag failure"));
        // }
        // tracing::info!("image tagged");

        // push image to local repo
        // let image_tag = format!("dominicburkart/{}:latest", function_name.clone());
        // let push_status = Command::new("docker")
        //     .arg("push")
        //     .arg(image_tag.clone())
        //     .status()?;
        // tracing::info!("docker push command did not explode");
        // if !push_status.success() {
        //     return Err(anyhow::anyhow!("docker image push failure"));
        // }

        // println!("haha >:D {}", image_tag);

        Ok(unique_tag.clone())
    })();
    println!("removing build dir");
    // always remove the build directory, even on build error
    std::fs::remove_dir_all(build_dir_canonical)?;
    println!("returning result");

    Command::new("kind")
        .args(
            format!("load docker-image {}", unique_tag)
                .as_str()
                .split(' '),
        )
        .status()?;
    // todo ^ temp fix while debugging kind
    result
}

impl Drop for K8s {
    #[tracing::instrument]
    fn drop(&mut self) {
        // todo

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

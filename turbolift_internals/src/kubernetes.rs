use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::str::FromStr;

use async_trait::async_trait;
use derivative::Derivative;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Service;
use kube::api::{Api, PostParams};
use kube::Client;
use regex::Regex;
use tokio::time::{sleep, Duration};
use tokio_compat_02::FutureExt;
use url::Url;
use uuid::Uuid;

use crate::distributed_platform::{
    ArgsString, DistributionPlatform, DistributionResult, JsonResponse,
};
use crate::utils::{DEBUG_FLAG, RELEASE_FLAG};
use crate::CACHE_PATH;

const TURBOLIFT_K8S_NAMESPACE: &str = "default";
type ImageTag = String;
type DeployContainerFunction = Box<dyn Fn(&str) -> anyhow::Result<&str> + Send + 'static>;

pub const CONTAINER_PORT: i32 = 5678;
pub const SERVICE_PORT: i32 = 5678;
pub const EXTERNAL_PORT: i32 = 80;
pub const TARGET_ARCHITECTURE: Option<&str> = None;
// ^ todo: we want the user to be able to specify something like `Some("x86_64-unknown-linux-musl")`
//   during config, but right now that doesn't work because we are relying on super unstable
//   span features to extract functions into services. When we can enable statically linked
//   targets, we can use the multi-stage build path and significantly reduce the size.

#[derive(Derivative)]
#[derivative(Debug)]
/// `K8s` is the interface for turning rust functions into autoscaling microservices
/// using turbolift. It requires docker and kubernetes / kubectl to already be setup on the
/// device at runtime.
///
/// Access to the kubernetes cluster must be inferrable from the env variables at runtime
/// per kube-rs's
/// [try_default()](https://docs.rs/kube/0.56.0/kube/client/struct.Client.html#method.try_default).
pub struct K8s {
    max_scale_n: u32,
    fn_names_to_ips: HashMap<String, Url>,
    request_client: reqwest::Client,
    run_id: Uuid,

    #[derivative(Debug = "ignore")]
    /// A function called after the image is built locally via docker. deploy_container
    /// receives the tag for the local image (accessible in docker) and is responsible
    /// for making said image accessible to the target cluster. The output of
    /// deploy_container is the tag that Kubernetes can use to refer to and access the
    /// image throughout the cluster.
    ///
    /// Some examples of how this function can be implemented: uploading the image to
    /// the cluster's private registry, uploading the image publicly to docker hub
    /// (if the image is not sensitive), loading the image into KinD in tests.
    deploy_container: DeployContainerFunction,
}

impl K8s {
    /// returns a K8s object. If max is equal to 1, then autoscaling
    /// is not enabled. Otherwise, autoscale is automatically activated
    /// with cluster defaults and a max number of replicas *per distributed
    /// function* of `max`. Panics if `max` < 1.
    ///
    /// The deploy container function is used for making containers accessible
    /// to the cluster. See [`K8s::deploy_container`].
    #[tracing::instrument(skip(deploy_container))]
    pub fn with_deploy_function_and_max_replicas(
        deploy_container: DeployContainerFunction,
        max: u32,
    ) -> K8s {
        if max < 1 {
            panic!("max < 1 while instantiating k8s (value: {})", max)
        }
        K8s {
            deploy_container,
            max_scale_n: max,
            fn_names_to_ips: HashMap::new(),
            request_client: reqwest::Client::new(),
            run_id: Uuid::new_v4(),
        }
    }
}

fn sanitize_function_name(function_name: &str) -> String {
    function_name.to_string().replace("_", "-")
}

#[async_trait]
impl DistributionPlatform for K8s {
    #[tracing::instrument(skip(project_tar))]
    async fn declare(&mut self, function_name: &str, project_tar: &[u8]) -> DistributionResult<()> {
        // connect to cluster. tries in-cluster configuration first, then falls back to kubeconfig file.
        let deployment_client = Client::try_default().compat().await?;
        let deployments: Api<Deployment> =
            Api::namespaced(deployment_client, TURBOLIFT_K8S_NAMESPACE);
        let service_client = Client::try_default().compat().await?;
        let services: Api<Service> = Api::namespaced(service_client, TURBOLIFT_K8S_NAMESPACE);

        // generate image & push
        let app_name = format!("{}-{}", sanitize_function_name(function_name), self.run_id);
        let container_name = format!("{}-app", app_name);
        let deployment_name = format!("{}-deployment", app_name);
        let service_name = format!("{}-service", app_name);
        let ingress_name = format!("{}-ingress", app_name);
        let tag_in_reg = make_image(self, &app_name, function_name, project_tar)?;

        // make deployment
        let deployment_json = serde_json::json!({
            "apiVersion": "apps/v1",
            "kind": "Deployment",
            "metadata": {
                "name": deployment_name,
                "labels": {
                    "turbolift_run_id": self.run_id.to_string()
                }
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
                       "app": app_name,
                       "turbolift_run_id": self.run_id.to_string(),
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
        let deployment = serde_json::from_value(deployment_json)?;
        deployments
            .create(&PostParams::default(), &deployment)
            .compat()
            .await?;

        // make service pointing to deployment
        let service_json = serde_json::json!({
            "apiVersion": "v1",
            "kind": "Service",
            "metadata": {
                "name": service_name,
                "labels": {
                    "turbolift_run_id": self.run_id.to_string(),
                }
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
        services
            .create(&PostParams::default(), &service)
            .compat()
            .await?;

        // make ingress pointing to service
        let ingress = serde_json::json!({
            "apiVersion": "networking.k8s.io/v1",
            "kind": "Ingress",
            "metadata": {
                "name": ingress_name,
                "labels": {
                    "turbolift_run_id": self.run_id.to_string(),
                }
            },
            "spec": {
                "rules": [
                    {
                        "http": {
                            "paths": [
                                {
                                    "path": format!("/{}/{}", function_name, self.run_id),
                                    "pathType": "Prefix",
                                    "backend": {
                                        "service" : {
                                            "name": service_name,
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

        let ingress_ip = format!(
            "http://localhost:{}/{}/{}/",
            EXTERNAL_PORT, function_name, self.run_id
        ); // we assume for now that the ingress is exposed on localhost

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
                return Err(anyhow::anyhow!(
                    "autoscale error: error code: {:?}",
                    scale_status.code()
                )
                .into());
                // ^ todo attach error context from child
            }
        }

        sleep(Duration::from_secs(90)).await;
        // todo make sure that the pod and service were correctly started before returning
        // todo implement the check on whether the service is running / pod failed

        self.fn_names_to_ips
            .insert(function_name.to_string(), Url::from_str(&ingress_ip)?);
        Ok(())
    }

    #[tracing::instrument]
    async fn dispatch(
        &mut self,
        function_name: &str,
        params: ArgsString,
    ) -> DistributionResult<JsonResponse> {
        // request from server
        let service_base_url = self.fn_names_to_ips.get(function_name).unwrap();
        let args = format!("./{}", params);
        let query_url = service_base_url.join(&args)?;
        tracing::info!(url = query_url.as_str(), "sending dispatch request");
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
        self.fn_names_to_ips.contains_key(fn_name)
    }
}

lazy_static! {
    static ref PORT_RE: Regex = Regex::new(r"0\.0\.0\.0:(\d+)->").unwrap();
}

#[tracing::instrument(skip(project_tar))]
fn make_image(
    k8s: &K8s,
    app_name: &str,
    function_name: &str,
    project_tar: &[u8],
) -> anyhow::Result<ImageTag> {
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
                    "RUN cargo build{release_flag}
                     CMD cargo run{release_flag} -- 0.0.0.0:{container_port}",
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

        Ok(unique_tag.clone())
    })();
    // always remove the build directory, even on build error
    std::fs::remove_dir_all(build_dir_canonical)?;

    result.and((k8s.deploy_container)(unique_tag.as_str()).map(|s| s.to_string()))
}

impl Drop for K8s {
    #[tracing::instrument]
    fn drop(&mut self) {
        let status = Command::new("kubectl")
            .args(
                format!(
                    "delete pods,deployments,services,ingress -l turbolift_run_id={}",
                    self.run_id.to_string()
                )
                .split(' '),
            )
            .status()
            .expect("could not delete Kubernetes resources");
        if !status.success() {
            eprintln!("could not delete Kubernetes resources")
        }
    }
}

#!/usr/bin/env sh

# assumes cargo, rust nightly, kind, and kubectl are installed. run from turbolift/examples/kubernetes_example

# error if any command fails
set -e

echo "🚡 running turbolift tests..."

printf "\n😤 deleting current cluster if it exists\n"
kind delete cluster

printf "\n📍 running non-distributed tests\n"
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run
echo "non-distributed tests completed."

printf "\n👷 setting up cluster with custom ingress-compatible config\n"
cat <<EOF | kind create cluster --config=-
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  kubeadmConfigPatches:
  - |
    kind: InitConfiguration
    nodeRegistration:
      kubeletExtraArgs:
        node-labels: "ingress-ready=true"
  extraPortMappings:
  - containerPort: 80
    hostPort: 80
    protocol: TCP
  - containerPort: 443
    hostPort: 443
    protocol: TCP
EOF
kubectl cluster-info --context kind-kind
echo "cluster initialized."

printf "\n🚪 adding ingress\n"
. setup_ingress.sh

echo "🚪 ingress ready."

printf "\n🤸‍ run distributed tests\n"
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test --features distributed -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run --features distributed
echo "🤸 distributed tests completed."

printf "\n📍 re-run non-distributed tests\n"
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run
echo "📍 non-distributed tests completed."

printf "\n🚡turbolift tests complete.\n"

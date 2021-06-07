#!/usr/bin/env sh

# assumes cargo, rust nightly, kind, and kubectl are installed. run from turbolift/examples/kubernetes_example

# error if any command fails
set -e

echo "🚡 running turbolift tests..."

printf "\n😤 deleting current cluster if it exists\n"
kind delete cluster

printf "\n📍 running non-distributed tests\n"
. ./run_non_distributed_tests.sh
echo "non-distributed tests completed."

printf "\n🚽 deleting target folder to save space in CI\n"
rm -r ./target
echo "target folder deleted"

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
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/master/deploy/static/provider/kind/deploy.yaml
sleep 90s # give k8s time to generate the pod ^
printf "\n⏱️ waiting for ingress controller to be ready...\n"
kubectl wait --namespace ingress-nginx \
  --for=condition=ready pod \
  --selector=app.kubernetes.io/component=controller \
  --timeout=30m

echo "🚪 ingress ready."

printf "\n🤸‍ run distributed tests\n"
. ./run_distributed_tests.sh
echo "🤸 distributed tests completed."

printf "\n🚽 deleting target folder to save space in CI\n"
rm -r ./target
echo "target folder deleted"

printf "\n📍 re-run non-distributed tests\n"
. ./run_non_distributed_tests.sh
echo "📍 non-distributed tests completed."

printf "\n🚡turbolift tests complete.\n"

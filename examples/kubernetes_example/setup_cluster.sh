#!/usr/bin/env sh

# assumes that kind is already setup

printf "\n😤 deleting current cluster if it exists\n"
kind delete cluster

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

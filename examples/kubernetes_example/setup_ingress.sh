#!/usr/bin/env sh
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/master/deploy/static/provider/kind/deploy.yaml
sleep 90s # give k8s time to generate the pod ^
printf "\n⏱️ waiting for ingress controller to be ready...\n"
kubectl wait --namespace ingress-nginx \
  --for=condition=ready pod \
  --selector=app.kubernetes.io/component=controller \
  --timeout=30m
kubectl describe pods -A
kubectl logs --all-containers=true -l app.kubernetes.io/component=controller

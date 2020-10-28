#!/usr/bin/env sh

# error if any command fails
set -e

# shellcheck disable=SC2139
alias kind="$PWD/kind"
# shellcheck disable=SC2139
alias kubectl="$PWD/kubectl"

cd turbolift/examples/kubernetes_example

# run non-distributed example without cluster in environment
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run

# setup cluster (will be used in all tests & runs)
kind create cluster --wait 20m
kubectl cluster-info --context kind-kind
kubectl get ns

# run non-distributed tests again with cluster in environment
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run

# run distributed tests
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test --features distributed -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run --features distributed

kind delete cluster

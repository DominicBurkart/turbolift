#!/usr/bin/env sh

# assumes cargo, rust nightly, microk8s, and kubectl are installed. run from turbolift/examples/kubernetes_example
# microk8s needs to have the local registry feature.

# error if any command fails
set -e

# stop microk8s
microk8s stop

# run non-distributed tests without cluster
. ./run_non_distributed_tests.sh

# start cluster
microk8s start
microk8s status --wait-ready

# re-run non-distributed tests
. ./run_non_distributed_tests.sh

# run distributed tests
. ./run_distributed_tests.sh

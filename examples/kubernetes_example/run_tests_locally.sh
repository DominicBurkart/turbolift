#!/usr/bin/env sh

# assumes cargo, rust nightly, kind, and kubectl are installed. run from turbolift/examples/kubernetes_example

# error if any command fails
set -e

# run non-distributed tests without cluster
. ./run_non_distributed_tests.sh

# generate cluster
kind create cluster

# re-run non-distributed tests
. ./run_non_distributed_tests.sh || kind delete cluster

# run distributed tests
. ./run_distributed_tests.sh || kind delete cluster

# delete cluster
kind delete cluster

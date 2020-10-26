#!/usr/bin/env sh

# error if any command fails
set -e

cd turbolift/examples/kubernetes_example

# run non-distributed example without cluster in environment
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run

# setup cluster (will be used in all tests & runs)
../../../kind create cluster

# run same tests with cluster in environment
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run

# run distributed tests
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test --features distributed -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run --features distributed

../../../kind delete cluster

#!/usr/bin/env sh

# assumes cargo, rust nightly, kind, and kubectl are installed. run from turbolift/examples/kubernetes_example

# error if any command fails
set -e

echo "🚡 running turbolift tests..."

printf "\n😤 deleting current cluster if it exists\n"
kind delete cluster # make sure we don't need the cluster when running locally

printf "\n📍 running non-distributed tests\n"
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run
echo "non-distributed tests completed."

. setup_cluster.sh

printf "\n🤸‍ run distributed tests\n"
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test --features distributed -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run --features distributed
echo "🤸 distributed tests completed."

printf "\n📍 re-run non-distributed tests\n"
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run
echo "📍 non-distributed tests completed."

printf "\n🚡turbolift tests complete.\n"

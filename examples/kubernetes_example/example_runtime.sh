# error if any command fails
set -e

# setup cluster (will be used in all tests & runs)
cd examples/kubernetes_example

# run non-distributed example without cluster in environment
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run

../../kind create cluster

# run same tests with cluster in environment
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run

# run distributed tests
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test --features distributed
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run --features distributed

../../kind delete cluster
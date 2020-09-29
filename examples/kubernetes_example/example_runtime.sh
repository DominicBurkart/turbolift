# error if any command fails
set -e

# setup cluster (will be used in all tests & runs)
./kind create cluster

cd examples/kubernetes_example

# test
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test --features "distributed"

# run
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run --features "distributed"
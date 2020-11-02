#!/usr/bin/env sh

# error if any command fails
set -e

# run distributed tests
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test --features distributed -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run --features distributed

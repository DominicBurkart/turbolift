#!/usr/bin/env sh

# error if any command fails
set -e

# run non-distributed tests
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo test -- --nocapture
RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo run
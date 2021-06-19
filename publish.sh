#!/usr/bin/env sh

set -e

cd ./turbolift_internals && RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo publish
sleep 70
cd ../turbolift_macros && RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo publish
sleep 70
cd .. && RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo publish

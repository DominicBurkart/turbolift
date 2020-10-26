# run me in turbolift root! E.G.: "docker build -f examples/local_queue_example/Dockerfile ."
FROM rustlang/rust:nightly
COPY ./ turbolift
WORKDIR turbolift/examples/local_queue_example

# test
RUN RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test -- --nocapture
RUN RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test --features distributed -- --nocapture

# run
RUN RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run
RUN RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly run --features distributed

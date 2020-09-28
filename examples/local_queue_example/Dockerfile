# run me in turbolift root! E.G.: "docker -f examples/local_queue_example/Dockerfile ."
FROM rustlang/rust:nightly
COPY ./ ./
WORKDIR examples/local_queue_example
CMD RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo +nightly test
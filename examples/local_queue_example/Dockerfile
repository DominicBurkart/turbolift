# run me in turbolift root! E.G.: "docker build -f examples/local_queue_example/Dockerfile ."
FROM rustlang/rust:nightly
RUN rustup default nightly-2020-09-28
ENV RUSTFLAGS='--cfg procmacro2_semver_exempt'
COPY ./ turbolift
WORKDIR turbolift/examples/local_queue_example

# test
RUN cargo +nightly test -- --nocapture
RUN cargo +nightly test --features distributed -- --nocapture

# run
RUN cargo +nightly run
RUN cargo +nightly run --features distributed

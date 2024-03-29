# Turbolift

[![crates.io](https://img.shields.io/crates/v/turbolift.svg)](https://crates.io/crates/turbolift)
[![docs.rs](https://img.shields.io/docsrs/turbolift.svg)](https://docs.rs/turbolift)
[![status](https://img.shields.io/github/workflow/status/dominicburkart/turbolift/examples)](https://github.com/DominicBurkart/turbolift/actions?query=branch%3Amain)
[![last commit](https://img.shields.io/github/last-commit/dominicburkart/turbolift)](https://github.com/DominicBurkart/turbolift)
[![website](https://img.shields.io/badge/-website-blue)](https://dominic.computer/turbolift)

Turbolift is a prototype distribution interface for rust. It's designed to make
distribution easier and more maintainable by extracting and distributing specific
functions and their dependencies from a larger rust application.
Turbolift then acts as the glue between these extracted mini-apps and
the main application.

Look in the [examples](https://github.com/DominicBurkart/turbolift/tree/main/examples)
directory for full projects with working syntax examples. An [external example](https://github.com/DominicBurkart/turbolift_example)
is maintained that can be used as a template repo.

## Notes

- Turbolift works as a proof-of-concept, but has not been optimized to shrink compilation time/requirements.
- Distribution is feature-gated in Turbolift to facilitate development / conditional distribution. The feature is called "distributed."
- Turbolift is implemented over http using `reqwest` and `actix-web` (no current plans to
refactor to use a lower level network protocol).
- Turbolift assumes a secure network– function parameters are sent in plaintext to the
microservice.
- When building, anything in the project directory or in
local dependencies declared in the project manifest could be bundled and sent
over the network to workers.

More information is available on the [project homepage](https://dominic.computer/turbolift).

## Current Limitations

- *Because of reliance on unstable proc_macro::Span features, all programs
using turbolift need to be built with an unstable nightly compiler flag (e.g.
`RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo build`)*
([tracking issue](https://github.com/rust-lang/rust/issues/54725)).
- Functions are assumed to be pure (lacking side-effects such as
writing to the file system or mutation of a global variable).
- For a function to be distributed, its inputs and outputs have to be
(de)serializable with [Serde](https://github.com/serde-rs/serde).
- Distributed functions cannot be nested in other functions.
- Distributed functions cannot be methods.
- Distributed functions cannot use other functions called `main`.
- Distributed functions not in `main.rs` cannot use functions declared
in `main.rs`.
- Distributed functions cannot have `-> impl Trait` types.
- Unused functions that have been marked with the `on` macro will still be
compiled for distribution, even if eventually the linker will then
remove the completed binary and distribution code.
- projects can have relative local dependencies listing in the cargo
manifest, but those dependencies themselves should not have relative local
dependencies prone to breaking.
- if your program produces side effects when initialized, for example when
global constants are initialized, those side effects may be triggered
for each function call.
- turbolift runs functions on an unreproducible linux build, it doesn't
e.g. pin the env or match the OS of the current environment.

## Current Project Goals

- [X] support kubernetes ([pr](https://github.com/DominicBurkart/turbolift/pull/2)).
- [X] implement startup, liveliness, and readiness probes for pods.
- [ ] while setting up a new service, wait for the pod to come alive via
readiness check instead of just sleeping ([code location](https://github.com/DominicBurkart/turbolift/blob/6a63d09afcd6e7234e62bcb797d31730cf49aacf/turbolift_internals/src/kubernetes.rs#L257)).
- [ ] roadmap support for other targets.
- [X] only use distributed configuration when flagged (like in
`cargo build --features "distributed"`). Otherwise, just transform the
tagged function into an async function (to provide an identical API), but
don't build any microservices or alter any code.
- [ ] build cross-architecture compilation tests into the CI.

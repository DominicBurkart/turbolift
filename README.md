# Turbolift

<img
    src="https://img.shields.io/crates/v/turbolift.svg"
    alt="turbolift’s current version badge"
    title="turbolift’s current version badge" />
[![status](https://github.com/DominicBurkart/turbolift/workflows/rust/badge.svg)](https://github.com/DominicBurkart/turbolift/actions?query=is%3Acompleted+branch%3Amaster+workflow%3A"rust")
[![status](https://github.com/DominicBurkart/turbolift/workflows/docker/badge.svg)](https://github.com/DominicBurkart/turbolift/actions?query=is%3Acompleted+branch%3Amaster+workflow%3A"docker")

## Distribution as an afterthought.
Turbolift allows developers to turn normal rust functions into distributed services 
 just by tagging them with a macro. This lets you develop in a monolith environment, 
but benefit from the scalability of microservice architectures. This pattern allows 
the compiler to help with the "hidden" complexity of connecting microservices, in 
exchange for longer compile times and larger dependency caches compared to non-distributed systems.

If you need to build or run your application without distribution, 
just pass the `local` feature: ```cargo run --features "local"```. The functions tagged for 
distribution will have identical signatures to the production version, but will run locally 
when you `.await` them.

## Important implementation notes
- implemented over http using `reqwest` and `actix-web` (no current plans to refactor to use a lower level network protocol).
- assumes a secure network– function parameters are sent in plaintext to the microservice.
- source vulnerability: when building, anything in the project directory is bundled and sent over the network to workers. 

## Current Limitations
- *Because of reliance on unstable proc_macro::Span features, all programs using turbolift need to 
be built with an unstable nightly compiler flag (e.g. `RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo build`)* ([tracking issue](https://github.com/rust-lang/rust/issues/54725)).
- Functions are assumed to be pure (lacking side-effects such as 
writing to the file system or mutation of a global variable). 
*Today, this is not enforced by the code.* 
- For a function to be distributed, its inputs and outputs have to be (de)serializable with [Serde](https://github.com/serde-rs/serde).
- Distributed functions cannot be nested in other functions.
- Distributed functions cannot be methods.
- Distributed functions cannot use other functions called `main`.
- Distributed functions not in `main.rs` cannot use functions declared 
in `main.rs`.
- Distributed functions cannot have `-> impl Trait` types.
- Unused functions that have been marked with the `on` macro will still be 
compiled for distribution, even if eventually the linker will then 
remove the completed binary and distribution code.
- *Turbolift doesn't match the cargo compilation settings for microservices yet.*
- projects can have relative local dependencies listing in the cargo manifest, but those dependencies themselves 
should not have relative local dependencies prone to breaking.

## Project Goals
- [ ] support kubernetes.
- [ ] roadmap support for other targets.
- [ ] disable distribution using `cargo build --features "local"`, causing each "distributed" function to simply be
transformed into an async function.
- [x] don't send debug build artifacts. 
- [x] don't unnecessarily clone the stringified function name every time that a function call is dispatched to to the distribution platform.
- [ ] build cross-architecture compilation tests into the CI.

## Tech debt todo
- [ ] refactor rust-embed / tar situation (we don't need to use both)
- [x] address compiler warnings (mostly about unnecessarily mutable values & unused vars) 
- [x] restructure project & re-export turbolift_macros from parent lib (turbolift)
- [ ] start reducing ginormous API, right now basically everything is public

## Notes
- if your program produces side effects when initialized, for example when 
global constants are initialized, those side effects may be triggered 
for each function call.
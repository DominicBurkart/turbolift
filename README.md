# Turbolift

<img
    src="https://img.shields.io/crates/v/turbolift.svg"
    alt="turbolift’s current version badge"
    title="turbolift’s current version badge" />
[![status](https://github.com/DominicBurkart/turbolift/workflows/rust/badge.svg)](https://github.com/DominicBurkart/turbolift/actions?query=is%3Acompleted+branch%3Amaster+workflow%3A"rust")
[![status](https://github.com/DominicBurkart/turbolift/workflows/docker/badge.svg)](https://github.com/DominicBurkart/turbolift/actions?query=is%3Acompleted+branch%3Amaster+workflow%3A"docker")

Turbolift is a WIP distribution platform for rust. It's designed to make distribution an afterthought 
by extracting and distributing specific functions and their dependencies from a larger rust application.
Turbolift then acts as the glue between these extracted mini-apps and the main application.

Look in the [examples](https://github.com/DominicBurkart/turbolift/tree/master/examples) directory for 
full projects with working syntax examples. 

## Distribution as an afterthought.
Turbolift allows developers to turn normal rust functions into distributed services 
 just by tagging them with a macro. This lets you develop in a monorepo environment, 
but benefit from the scalability of microservice architectures.

## Orchestration with a feature flag.
For quicker development builds, `cargo build` doesn't build the distributed version of your code. 
Instead, the functions tagged for distribution will have identical signatures to the production version, 
but will run locally when you `.await` them. Same as we use `--release` for better optimization, 
we use `--features "distributed"` to build the relevant orchestration: `cargo build --release --features "distributed"`.

## Important implementation notes
- implemented over http using `surf` and `actix-web` (no current plans to refactor to use a lower level network protocol).
- assumes a secure network– function parameters are sent in plaintext to the microservice.
- source vulnerability: when building, anything in the project directory or in local dependencies 
declared in the project manifest could be bundled and sent over the network to workers. 

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
- if your program produces side effects when initialized, for example when 
global constants are initialized, those side effects may be triggered 
for each function call.

## Current Project Goals
- [ ] support kubernetes ([pr](https://github.com/DominicBurkart/turbolift/pull/2)).
- [ ] roadmap support for other targets.
- [X] only use distributed configuration when flagged (like in `cargo build --features "distributed"`). Otherwise,
just transform the tagged function into an async function (to provide an identical API), but don't 
build any microservices or alter any code.
- [ ] build cross-architecture compilation tests into the CI (RN we only test via github actions read Docker, and a different custom Docker test workflow)

## Current tech debt todo
- [ ] start reducing ginormous API, right now basically everything is public
- [ ] refactor split between turbolift_internals and turbolift_macros
- [ ] improve names

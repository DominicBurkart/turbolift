# Turbolift

<img
    src="https://img.shields.io/crates/v/turbolift.svg"
    alt="turbolift’s current version badge"
    title="turbolift’s current version badge" />
[![status](https://github.com/DominicBurkart/turbolift/workflows/rust%20linters/badge.svg)](https://github.com/DominicBurkart/turbolift/actions?query=is%3Acompleted+branch%3Amaster+workflow%3A"rust+linters")

Distributing rust programs, function by function. NOTE: Turbolift is 
still very much in development. The readme below is as much proscriptive 
as prescriptive.

## Example

With Turbolift, you tag every function that should be distributed 
 with an [attribute macro](https://doc.rust-lang.org/reference/procedural-macros.html#attribute-macros). Below 
 is an example of how `on` should work.


### Before (not distributed)
```rust
// imports
use fastrand;

// local function
fn expensive_calculation(u: u32) -> u32 {
    u.pow(u)
}

fn main() {
    // run the expensive function on 100 random inputs and sum results
    let sum = 0..100
        .map(
            |_| {
                let u = fastrand::u32(0..10);
                expensive_calculation(u)
            }
        )
        .sum();
        
    // print sum
    println!("sum: {}", sum);
}
```

### distributed via [Swarm](https://docs.docker.com/engine/swarm/)
```rust
// imports
use fastrand;
use std::net::{Ipv4Addr};
use turbolift_macros::on;
use turbolift::{Swarm, SwarmParams};
use futures::executor::block_on;
use futures::future::try_join_all;

// declare any distribution platforms (here SWARM) once in your project. 
const SWARM: Swarm = Swarm::from_params(
    SwarmParams {
        host: Ipv4Addr::new(192, 168, 0, 1),
        port: 5000,
        ..SwarmParams::default()
    }
);

// for every function you want to distribute, add the `on` macro to 
// set the distribution platform.
#[on(SWARM)]
fn expensive_calculation(u: u32) -> u32 {
    u.pow(u)
}


fn main() {
    // run the (distributed) expensive function on 100 random inputs
    let mut futures = Vec::new();
    for _ in 0..100 {
        let u = fastrand::u32(0..10);
        futures.push(expensive_calculation(u));
    }
    
    // collect results
    let results = block_on(try_join_all(futures)).unwrap();
    
    // sum results
    let sum = results
        .into_iter()
        .map(|res| res.unwrap())
        .sum();
    
    // print sum
    println!("sum: {}", sum);
}
```

### What are the differences?
- `expensive_calculation` can now be run as a microservice on any node in the swarm,
and every call to the function will trigger a HTTP request to one of the services.
- `expensive_calculation` is now async. It returns a `DistributedResult<u32>`,
instead of directly returning `u32`.
- note that we switched from a lazy execution scheme (using `map`) to 
an eager scheme (using a loop), so that tasks were dispatched to the 
distribution platform as soon as possible.

## Distribution as an afterthought.
Turbolift allows developers to turn normal rust functions into distributed services 
 just by tagging them with a macro. This:
- lowers the barrier of entry for distributing a program.
- decreases tech debt and switching costs associated with distribution.
- lets you organize your project according to application logic, not orchestration 
details.

This architecture is of course not optimal for all projects, but can be useful 
for certain applications, especially computation-heavy batch jobs.

## Supported Distribution Platforms
- local queue 
- (other targets WIP, starting with Docker Swarm)

## Current Limitations
- *Instability. because of reliance on unstable proc_macro::Span features, all programs using turbolift need to 
be built with an unstable nightly compiler flag (e.g. `RUSTFLAGS='--cfg procmacro2_semver_exempt' cargo build`)* ([tracking issue](https://github.com/rust-lang/rust/issues/54725)).
- Functions are assumed to be pure (lacking side-effects such as 
writing to the file system or mutation of a global variable). 
*Today, this is not enforced by the code.* 
- For a function to be distributed, its inputs and outputs have to be serializable with [Serde](https://github.com/serde-rs/serde).
- Distributed functions cannot be nested in other functions.
- Distributed functions cannot be methods.
- Distributed functions cannot use other functions called `main`.
- Distributed functions not in `main.rs` cannot use functions declared 
in `main.rs`.
- Distributed functions cannot have `-> impl Trait` types.
- Unused functions that have been marked with the `on` macro will still be 
compiled for distribution, even if eventually the linker will then 
remove the completed binary and distribution code.
- *Turbolift doesn't match the cargo compilation settings yet.*

## Notes
- if your program produces side effects when initialized, for example when 
global constants are initialized, those side effects may be triggered 
for each function call.
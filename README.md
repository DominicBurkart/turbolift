# Turbolift

Distributing rust programs, function by function. NOTE: Turbolift is 
under active development, but is not yet ready for production use.

## Example

With Turbolift, you tag a every function that should be distributed 
 with the [attribute macro](https://doc.rust-lang.org/reference/procedural-macros.html#attribute-macros) `on`.
 `on` takes a distribution platform as an argument, and turns the 
 function into a portable application binary that can be run on the 
 passed platform. 


Here is an example of a local function (before):
```rust
// imports
use fastrand;

// local function
fn expensive_calculation(u: u128) -> u128 {
    u.pow(u)
}

fn main() {
    // run the expensive function on 100 random inputs and sum results
    let sum = 0..100
        .map(
            |_| {
                let u = fastrand::u128(0..10);
                expensive_calculation(u)
            }
        )
        .sum();
        
    // print sum
    println!("sum: {}", sum);
}
```

and here is an example of a function set to run as a service on [Swarm](https://docs.docker.com/engine/swarm/) (after):
```rust
// imports
use fastrand;
use std::net::{Ipv4Addr};
use turbolift::{on, Swarm, SwarmParams};
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
fn expensive_calculation(u: u128) -> u128 {
    u.pow(u)
}


fn main() {
    // run the (distributed) expensive function on 100 random inputs
    let mut futures = Vec::new();
    for _ in 0..100 {
        let u = fastrand::u128(0..10);
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
- `expensive_calculation` can now be run on any node in the swarm, and 
every call to the function will trigger a swarm task.
- `expensive_calculation` now returns `Future<DistributedResult<u128>>`,
instead of returning `u128`.
- note that we switched from a lazy execution scheme (using `map`) to 
an eager scheme (using a loop), so that tasks were dispatched to the 
distribution platform as soon as possible.

## Features
- distribution as an afterthought. By distributing functions using 
a simple macro, you can organize your project based on the application 
logic, instead of structuring your code based on where it will run. 
Instead of breaking up your code and tailoring the components to work on
a specific distribution platform, you can use regular rust syntax. This 
makes adding distribution to an existing program easier, and abstracting 
away from the target platform decreases tech debt and makes switching 
(or mixing) platforms much, much easier.
- resource control when you want it. The `on` macro provides the simplest interface
possible, giving basic distribution. The `with` macro requires more information, but
lets the distribution platforms handle concurrency way better and provide 
quick warnings if the cluster is not set up with the correct resources 
to run a given task. 
- block when the distribution platform is overwhelmed. By default, 
 the number of tasks the distribution platform should process at a given 
 time is limited (and can be set by the programmer). When the limit is 
 reached and the platform is busy, instead of quickly returning a future 
 as normal, a distributed function call will block until a task has 
 completed and the distribution platform can accept another task. 

## Supported Distribution Platforms
Currently, only Docker [Swarm](https://docs.docker.com/engine/swarm/)
and a local debug queue are targeted. I'd love help adding additional
target platforms! Platforms I would especially love to support:
- AWS Lambda
- Kubernetes
- Apache Mesos (and/or Hadoop, and/or Spark)

Some cluster managers and task schedulers that can't handle open-ended 
tasks could still support the `with` macro (like `on`, but forces the 
user to allocate minimum resource requirements for a function to be 
distributed). I would especially like to support [SLURM](https://en.wikipedia.org/wiki/Slurm_Workload_Manager).

## Current Limitations
- For a function to be distributed, its inputs and outputs have to be serializable with [Serde](https://github.com/serde-rs/serde).
- Distributed functions cannot be nested in other functions.
- Distributed functions cannot be methods.
- Distributed functions cannot use other functions called `main`.
- Distributed functions not in `main.rs` cannot use functions declared 
in `main.rs`.
- Unused functions that have been marked with the `on` macro will still be 
compiled for distribution, even if eventually the linker will then 
remove the completed binary and distribution code.
- Functions are assumed to be pure (lacking side-effects such as 
writing to the file system or mutation of a global variable). 
*Today, this is not enforced by the code.* 
- *Turbolift doesn't match the cargo compilation settings yet.*

## Notes
- if your program produces side effects when initialized, for example when 
global constants are initialized, those side effects may be triggered 
for each function call.
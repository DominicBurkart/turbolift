#![feature(async_closure)] // not actually necessary, just added to test that feature defs work.
#[macro_use]
extern crate lazy_static;
#[macro_use(c)]
extern crate cute;
use futures::future::try_join_all;
use rand::{thread_rng, Rng};
use tokio::sync::Mutex;
use tracing;
use tracing_subscriber;

use turbolift::kubernetes::K8s;
use turbolift::on;

/// instantiate the global cluster manager
lazy_static! {
    static ref K8S: Mutex<K8s> = Mutex::new(K8s::new(Box::new(load_container_into_kind), 2));
}

/// The application writer is responsible for placing
/// images where your cluster can access them. The
/// K8s constructor has a parameter which takes
/// a function that is called after the container is
/// built, so that the container may be added to a
/// specific registry or otherwise be made available.
fn load_container_into_kind(tag: String) -> anyhow::Result<String> {
    std::process::Command::new("kind")
        .args(format!("load docker-image {}", tag).as_str().split(' '))
        .status()?;
    Ok(tag)
}

/// This is the function we want to distribute!
#[on(K8S)]
fn square(u: u64) -> u64 {
    u * u
}

fn random_numbers() -> Vec<u64> {
    let mut pseud = thread_rng();
    c![pseud.gen_range(0, 1000), for _i in 1..10]
}

fn main() {
    // use tracing.rs to print info about the program to stdout
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(std::io::stdout)
        .init();

    let input = random_numbers();
    let futures = c![square(*int), for int in &input];
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(try_join_all(futures)).unwrap();
    println!(
        "\n\ncomputation complete.\ninput: {:?}\noutput: {:?}",
        input, output
    );
    if output != c![x*x, for x in input] {
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let input = random_numbers();
        let futures = c![square(*int), for int in &input];
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let output = rt.block_on(try_join_all(futures)).unwrap();
        assert_eq!(
            output,
            input.into_iter().map(|x| x * x).collect::<Vec<u64>>()
        );
    }
}

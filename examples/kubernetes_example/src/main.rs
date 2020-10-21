use std::sync::Mutex;

#[macro_use]
extern crate lazy_static;
#[macro_use(c)]
extern crate cute;
use futures::future::try_join_all;
use rand::{thread_rng, Rng};
use turbolift::kubernetes::K8s;
use turbolift::on;

lazy_static! {
    static ref K8S: Mutex<K8s> = Mutex::new(K8s::with_max_replicas(2));
}

#[on(K8S)]
fn square(u: u64) -> u64 {
    u * u
}

fn random_numbers() -> Vec<u64> {
    let mut pseud = thread_rng();
    c![pseud.gen_range(0, 1000), for _i in 1..10]
}

fn main() {
    let input = random_numbers();
    let futures = c![square(*int), for int in &input];
    let mut rt = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("error starting runtime");
    let output = rt.block_on(try_join_all(futures)).unwrap();
    println!("input: {:?}\noutput: {:?}", input, output);
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
        let mut rt = tokio::runtime::Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .expect("error starting runtime");
        let output = rt.block_on(try_join_all(futures)).unwrap();
        assert_eq!(
            output,
            input.into_iter().map(|x| x * x).collect::<Vec<u64>>()
        );
    }
}

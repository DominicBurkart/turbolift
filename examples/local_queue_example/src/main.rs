use std::sync::Mutex;

extern crate proc_macro;
use async_std::task;
use futures::future::try_join_all;
use rand;
use turbolift::local_queue::LocalQueue;
use turbolift::on;
#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref LOCAL: Mutex<LocalQueue> = Mutex::new(LocalQueue::new());
}

#[on(LOCAL)]
fn identity(b: bool) -> bool {
    b
}

fn main() {
    let input = vec![rand::random(), rand::random(), rand::random()];
    let futures = {
        let mut v = Vec::new();
        for b in &input {
            v.push(identity(*b));
        }
        v
    };
    let output = task::block_on(try_join_all(futures)).unwrap();
    println!("input: {:?}\noutput: {:?}", input, output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let input = vec![rand::random(), rand::random(), rand::random()];
        let futures = {
            let mut v = Vec::new();
            for b in &input {
                v.push(identity(*b));
            }
            v
        };
        let output = task::block_on(try_join_all(futures)).unwrap();
        assert_eq!(input, output);
    }
}
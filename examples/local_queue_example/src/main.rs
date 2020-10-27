extern crate proc_macro;
use futures::future::try_join_all;
use rand;
use turbolift::local_queue::LocalQueue;
use turbolift::on;
#[macro_use]
extern crate lazy_static;
use tokio::sync::Mutex;

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
    let mut rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(try_join_all(futures)).unwrap();
    println!("input: {:?}\noutput: {:?}", input, output);
    if output != input {
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        println!("test started");
        let input = vec![rand::random(), rand::random(), rand::random()];
        let futures = {
            let mut v = Vec::new();
            for b in &input {
                v.push(identity(*b));
            }
            v
        };
        let mut rt = tokio::runtime::Runtime::new().unwrap();
        let output = rt.block_on(try_join_all(futures)).unwrap();
        assert_eq!(input, output);
    }
}

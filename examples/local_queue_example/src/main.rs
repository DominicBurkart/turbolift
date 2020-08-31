use futures::executor::block_on;
use futures::future::try_join_all;
use turbolift_macros::on;
use turbolift::local_queue::LocalQueue;

const LOCAL: LocalQueue = Default::default();

#[on(LOCAL)]
fn ident(b: bool) -> bool {
    b
}

fn main() {
    let input = vec![rand::random(), rand::random(), rand::random()];
    let futures = {
        let mut v = Vec::new();
        for b in input {
            v.push(ident(b));
        }
        v
    };
    let result = block_on(try_join_all(futures)).unwrap();
    println!("input: {:#?}\nresult: {:#?}", input, result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let input = vec![rand::random(), rand::random(), rand::random()];
        let futures = {
            let mut v = Vec::new();
            for b in input {
                v.push(ident(b));
            }
            v
        };
        let result = block_on(try_join_all(futures)).unwrap();
        assert_eq!(
            input,
            result
        );
    }
}
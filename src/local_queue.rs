extern crate proc_macro;
use proc_macro::TokenStream;

use quote::quote;
use syn::{self, DeriveInput};

use crate::distributed_platform::{DistributionPlatform, DistributionResult, DistributionError};

pub struct LocalQueue;

impl DistributionPlatform for LocalQueue {
    fn start(&mut self) -> DistributionResult<()> {
        Ok(())
    }

    fn started(&self) -> bool {
        false
    }

    #[proc_macro_attribute]
    fn dispatch(attr: TokenStream, item: TokenStream) -> TokenStream {
        let function: DeriveInput = syn::parse(item).unwrap();
        println!("{}", function.ident);
        // println!("{:#?}", function.attrs);
        quote!(
            println!("nice") // todo
        ).into()
    }
}

impl Drop for LocalQueue {
    fn drop(&mut self) {}
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        const LOCAL: LocalQueue = LocalQueue{};

        #[on(local)]
        fn ident(b: bool) -> bool {
            true
        }

        assert!(2 + 2, 4);
    }
}

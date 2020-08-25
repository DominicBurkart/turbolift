extern crate proc_macro;
use proc_macro::TokenStream;

use syn::{parse_macro_input, Result, DeriveInput};
use syn::parse::{Parse, ParseStream};

mod distributed_platform;
mod local_queue;

#[proc_macro_attribute]
pub fn on(attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn with(attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

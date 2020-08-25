extern crate proc_macro;
use proc_macro::TokenStream;

mod distributed_platform;
mod local_queue;

#[proc_macro_attribute]
pub fn on(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[proc_macro_attribute]
pub fn with(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

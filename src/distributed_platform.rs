extern crate proc_macro;
use proc_macro::TokenStream;

pub struct DistributionError;
pub type DistributionResult<T> = std::result::Result<T, DistributionError>;

pub trait DistributionPlatform: Drop {
    /// called once before functions are sent
    fn start(&mut self) -> DistributionResult<()>;

    fn started(&self) -> bool;

    /// called once when the function is declared. Turns
    /// the function into something like a microservice, where some external
    /// process is waiting to serve new inputs & outputs.
    #[proc_macro_attribute]
    fn dispatch(attr: TokenStream, item: TokenStream) -> TokenStream;
}
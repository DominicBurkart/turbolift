extern crate proc_macro;
use proc_macro2::TokenStream;
use std::future::Future;
use std::error;

use async_trait::async_trait;


pub type DistributionError = Box<dyn error::Error>;
pub type DistributionResult<T> = std::result::Result<T, DistributionError>;

pub type ArgsString = String;
pub type JsonResponse = String;

#[async_trait]
pub trait DistributionPlatform {
    /// declare a function
    fn declare(&mut self, function_name: &str, project_binary: Vec<u8>);

    // dispatch params to a function
    async fn dispatch(&mut self, function_name: String, params: ArgsString) -> JsonResponse;
}
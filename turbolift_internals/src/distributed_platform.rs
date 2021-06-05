extern crate proc_macro;
use async_trait::async_trait;
use std::error;
use uuid::Uuid;

pub type DistributionError = Box<dyn error::Error>;
pub type DistributionResult<T> = std::result::Result<T, DistributionError>;

pub type ArgsString = String;
pub type JsonResponse = String;

#[async_trait]
pub trait DistributionPlatform {
    /// declare a function
    async fn declare(
        &mut self,
        function_name: &str,
        run_id: Uuid,
        project_tar: &[u8],
    ) -> DistributionResult<()>;

    // dispatch params to a function
    async fn dispatch(
        &mut self,
        function_name: &str,
        params: ArgsString,
    ) -> DistributionResult<JsonResponse>;

    fn has_declared(&self, fn_name: &str) -> bool;
}

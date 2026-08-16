pub mod mock;
mod niri;

pub use niri::*;

use async_trait::async_trait;
use niripip_core::{CompositorAction, CompositorEvent, OutputInfo};
use std::collections::HashMap;
use tokio::sync::mpsc;

#[async_trait]
pub trait NiriBackend: Send + Sync {
    async fn version(&self) -> Result<String, NiriIpcError>;
    async fn outputs(&self) -> Result<HashMap<String, OutputInfo>, NiriIpcError>;
    async fn execute(&self, action: CompositorAction) -> Result<(), NiriIpcError>;
    async fn subscribe(
        &self,
    ) -> Result<mpsc::Receiver<Result<CompositorEvent, NiriIpcError>>, NiriIpcError>;
}

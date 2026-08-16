use crate::{NiriBackend, NiriIpcError};
use async_trait::async_trait;
use niripip_core::{CompositorAction, CompositorEvent, OutputInfo};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct MockNiriBackend {
    version: String,
    outputs: HashMap<String, OutputInfo>,
    initial_events: Arc<Mutex<Vec<CompositorEvent>>>,
    actions: Arc<Mutex<Vec<CompositorAction>>>,
}

impl Default for MockNiriBackend {
    fn default() -> Self {
        Self {
            version: "26.04-mock".into(),
            outputs: HashMap::new(),
            initial_events: Arc::new(Mutex::new(Vec::new())),
            actions: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl MockNiriBackend {
    pub fn with_events(mut self, events: Vec<CompositorEvent>) -> Self {
        self.initial_events = Arc::new(Mutex::new(events));
        self
    }

    pub fn with_outputs(mut self, outputs: HashMap<String, OutputInfo>) -> Self {
        self.outputs = outputs;
        self
    }

    pub fn actions(&self) -> Vec<CompositorAction> {
        self.actions
            .lock()
            .expect("mock action mutex poisoned")
            .clone()
    }
}

#[async_trait]
impl NiriBackend for MockNiriBackend {
    async fn version(&self) -> Result<String, NiriIpcError> {
        Ok(self.version.clone())
    }

    async fn outputs(&self) -> Result<HashMap<String, OutputInfo>, NiriIpcError> {
        Ok(self.outputs.clone())
    }

    async fn execute(&self, action: CompositorAction) -> Result<(), NiriIpcError> {
        self.actions
            .lock()
            .expect("mock action mutex poisoned")
            .push(action);
        Ok(())
    }

    async fn subscribe(
        &self,
    ) -> Result<mpsc::Receiver<Result<CompositorEvent, NiriIpcError>>, NiriIpcError> {
        let events = self
            .initial_events
            .lock()
            .expect("mock event mutex poisoned")
            .clone();
        let (tx, rx) = mpsc::channel(events.len().max(1));
        tokio::spawn(async move {
            for event in events {
                if tx.send(Ok(event)).await.is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }
}

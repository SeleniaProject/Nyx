#![forbid(unsafe_code)]

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ty: String,
    pub detail: String,
}

/// Simple pub/sub for daemon events over a broadcast channel.
#[derive(Clone)]
pub struct EventSystem {
    tx: broadcast::Sender<Event>,
    // naive filter state; can evolve to per-subscriber rules
    default_types: Arc<RwLock<Vec<String>>>,
}

impl EventSystem {
    pub fn new(buffer: usize) -> Self {
        let (tx, _rx) = broadcast::channel(buffer);
        Self { tx, default_types: Arc::new(RwLock::new(vec!["system".into(), "metrics".into()])) }
    }

    pub fn sender(&self) -> broadcast::Sender<Event> { self.tx.clone() }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> { self.tx.subscribe() }

    pub async fn set_default_types(&self, types: Vec<String>) { *self.default_types.write().await = types; }

    pub async fn matches(&self, ev: &Event, filter: &Option<Vec<String>>) -> bool {
        let allow = match filter {
            Some(types) => types,
            None => &*self.default_types.read().await,
        };
        allow.iter().any(|t| t == &ev.ty)
    }
}

#![forbid(unsafe_code)]

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub _ty: String,
    pub _detail: String,
}

/// Simple pub/sub for daemon event_s over a broadcast channel.
#[derive(Clone)]
pub struct EventSystem {
    tx: broadcast::Sender<Event>,
    // naive filter state; can evolve to per-subscriber rule_s
    default_type_s: Arc<RwLock<Vec<String>>>,
}

impl EventSystem {
    pub fn new(buffer: usize) -> Self {
        let (tx, _rx) = broadcast::channel(buffer);
        Self {
            tx,
            default_type_s: Arc::new(RwLock::new(vec![
                "system".into(),
                "metric_s".into(),
                "power".into(),
            ])),
        }
    }

    pub fn sender(&self) -> broadcast::Sender<Event> {
        self.tx.clone()
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub async fn set_default_type_s(&self, type_s: Vec<String>) {
        *self.default_type_s.write().await = type_s;
    }

    pub async fn matches(&self, ev: &Event, filter: &Option<Vec<String>>) -> bool {
        let allow = match filter {
            Some(type_s) => type_s,
            None => &*self.default_type_s.read().await,
        };
        allow.iter().any(|t| t == &ev._ty)
    }
}

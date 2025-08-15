//! Simple push notification abstraction with pluggable providers (FCM/APNS, etc.).
//! This module defines a provider interface and a manager that can route messages
//! by topic to the appropriate provider while retaining an in-memory queue for
//! durability and observability in tests.
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct PushMessage {
    pub topic: String,
    pub payload: String,
    pub timestamp_ms: u128,
}

/// Provider-agnostic push message result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult {
    Accepted,
    Queued,
    Rejected(String),
}

/// Pluggable push provider interface
pub trait PushProvider: Send + Sync {
    /// Send a message to the remote push backend.
    fn send(&self, topic: &str, payload: &str, timestamp_ms: u128) -> PushResult;
    /// Lightweight health signal for provider readiness.
    fn healthy(&self) -> bool {
        true
    }
    /// Close/cleanup resources if necessary.
    fn close(&self) {}
}

/// A no-op FCM provider implementation (pure Rust, no external deps).
/// This simulates acceptance and can be extended to use a real backend via
/// feature gating when TLS dependencies are allowed.
pub struct FcmProvider {
    pub project_id: String,
    pub accept: bool,
}
impl PushProvider for FcmProvider {
    fn send(&self, _topic: &str, _payload: &str, _timestamp_ms: u128) -> PushResult {
        if self.accept {
            PushResult::Accepted
        } else {
            PushResult::Rejected("FCM provider not accepting".into())
        }
    }
    fn healthy(&self) -> bool {
        self.accept
    }
}

/// A no-op APNS provider implementation (pure Rust, no external deps).
pub struct ApnsProvider {
    pub team_id: String,
    pub accept: bool,
}
impl PushProvider for ApnsProvider {
    fn send(&self, _topic: &str, _payload: &str, _timestamp_ms: u128) -> PushResult {
        if self.accept {
            PushResult::Accepted
        } else {
            PushResult::Rejected("APNS provider not accepting".into())
        }
    }
    fn healthy(&self) -> bool {
        self.accept
    }
}

#[derive(Clone, Default)]
pub struct PushManagerInner {
    pub queue: VecDeque<QueuedItem>,
    pub max_queue: usize,
    pub providers: HashMap<String, Arc<dyn PushProvider>>, // name -> provider
    pub topic_routes: HashMap<String, String>,             // topic -> provider name
    pub retry_config: RetryConfig,
}

impl std::fmt::Debug for PushManagerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PushManagerInner")
            .field("queue_len", &self.queue.len())
            .field("max_queue", &self.max_queue)
            .field("providers", &self.providers.len())
            .field("topic_routes", &self.topic_routes.len())
            .finish()
    }
}

#[derive(Clone, Default)]
pub struct PushManager {
    inner: Arc<Mutex<PushManagerInner>>,
}

impl PushManager {
    /// Create a manager with an in-memory queue bound
    pub fn new(max_queue: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PushManagerInner {
                queue: VecDeque::new(),
                max_queue,
                providers: HashMap::new(),
                topic_routes: HashMap::new(),
                retry_config: RetryConfig::default(),
            })),
        }
    }

    /// Configure retry/backoff policy.
    pub fn with_retry_config(self, cfg: RetryConfig) -> Self {
        let mut g = self.inner.lock().unwrap();
        g.retry_config = cfg;
        drop(g);
        self
    }

    /// Register a provider by name. If a provider with the same name exists it will be replaced.
    pub fn register_provider(&self, name: &str, provider: Arc<dyn PushProvider>) {
        let mut g = self.inner.lock().unwrap();
        g.providers.insert(name.to_string(), provider);
    }

    /// Bind a topic to a provider name for routing.
    pub fn bind_topic(&self, topic: &str, provider_name: &str) {
        let mut g = self.inner.lock().unwrap();
        g.topic_routes
            .insert(topic.to_string(), provider_name.to_string());
    }

    /// Publish a message; routes to provider if bound, otherwise enqueues locally.
    pub fn publish(&self, topic: &str, payload: &str) {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let routed = {
            let g = self.inner.lock().unwrap();
            if let Some(provider_name) = g.topic_routes.get(topic) {
                if let Some(provider) = g.providers.get(provider_name) {
                    matches!(provider.send(topic, payload, ts), PushResult::Accepted)
                } else {
                    false
                }
            } else {
                false
            }
        };
        if !routed {
            self.enqueue(topic, payload, ts, 0);
        }
    }

    /// Drain the local queue (used by tests or fallback delivery loops).
    pub fn drain(&self) -> Vec<PushMessage> {
        let mut g = self.inner.lock().unwrap();
        g.queue.drain(..).map(|q| q.msg).collect()
    }

    /// Current local queue length.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().queue.len()
    }

    fn enqueue(&self, topic: &str, payload: &str, timestamp_ms: u128, attempts: u32) {
        let mut g = self.inner.lock().unwrap();
        if g.queue.len() >= g.max_queue {
            g.queue.pop_front();
        }
        let next = Instant::now();
        g.queue.push_back(QueuedItem {
            msg: PushMessage {
                topic: topic.into(),
                payload: payload.into(),
                timestamp_ms,
            },
            attempts,
            next_attempt_at: next,
        });
    }

    /// Start background delivery worker. It will retry queued messages with exponential backoff.
    pub fn start_worker(&self) -> tokio::task::JoinHandle<()> {
        let mgr = self.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            loop {
                tick.tick().await;
                let work = {
                    let mut g = mgr.inner.lock().unwrap();
                    // Peek front; if not yet due, skip this round
                    if let Some(front) = g.queue.front() {
                        if front.next_attempt_at > Instant::now() {
                            None
                        } else {
                            Some(g.queue.pop_front().unwrap())
                        }
                    } else {
                        None
                    }
                };
                if let Some(item) = work {
                    // Route
                    let (provider_opt, retry_cfg) = {
                        let g = mgr.inner.lock().unwrap();
                        let provider = g
                            .topic_routes
                            .get(&item.msg.topic)
                            .and_then(|name| g.providers.get(name))
                            .cloned();
                        (provider, g.retry_config)
                    };
                    if let Some(provider) = provider_opt {
                        match provider.send(
                            &item.msg.topic,
                            &item.msg.payload,
                            item.msg.timestamp_ms,
                        ) {
                            PushResult::Accepted => {
                                debug!("push delivered: topic={}", item.msg.topic);
                            }
                            PushResult::Queued => {
                                // Re-enqueue with backoff as if it were a soft failure.
                                mgr.requeue_with_backoff(item, retry_cfg);
                            }
                            PushResult::Rejected(err) => {
                                warn!(
                                    "push rejected by provider: topic={} err={} attempts={}",
                                    item.msg.topic, err, item.attempts
                                );
                                mgr.requeue_with_backoff(item, retry_cfg);
                            }
                        }
                    } else {
                        // No route/provider → re-enqueue with backoff
                        mgr.requeue_with_backoff(item, retry_cfg);
                    }
                }
            }
        })
    }

    fn requeue_with_backoff(&self, mut item: QueuedItem, cfg: RetryConfig) {
        item.attempts = item.attempts.saturating_add(1);
        let backoff_ms = ((cfg.base_delay_ms as u128)
            * (1u128 << item.attempts.min(cfg.max_exponent) as u128))
            .min(cfg.max_delay_ms as u128);
        let jitter = (backoff_ms as f64 * cfg.jitter).round() as u128;
        let next_at = Instant::now() + Duration::from_millis((backoff_ms + jitter) as u64);
        item.next_attempt_at = next_at;
        let mut g = self.inner.lock().unwrap();
        if g.queue.len() >= g.max_queue {
            g.queue.pop_front();
        }
        g.queue.push_back(item);
    }
}

/// Internal queue entry with retry/backoff metadata
#[derive(Clone, Debug)]
struct QueuedItem {
    msg: PushMessage,
    attempts: u32,
    next_attempt_at: Instant,
}

/// Retry/backoff configuration for push delivery
#[derive(Clone, Copy, Debug)]
pub struct RetryConfig {
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub max_exponent: u32,
    /// Jitter ratio (0.0..1.0)
    pub jitter: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            base_delay_ms: 250,
            max_delay_ms: 30_000,
            max_exponent: 8,
            jitter: 0.1,
        }
    }
}

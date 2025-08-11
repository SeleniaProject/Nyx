//! Simple push notification mock (FCM/APNS abstraction) for Section F.
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct PushMessage { pub topic: String, pub payload: String, pub timestamp_ms: u128 }

#[derive(Debug, Clone, Default)]
pub struct PushManagerInner { pub queue: VecDeque<PushMessage>, pub max_queue: usize }

#[derive(Clone, Default)]
pub struct PushManager { inner: Arc<Mutex<PushManagerInner>> }
impl PushManager {
    pub fn new(max_queue: usize) -> Self { Self { inner: Arc::new(Mutex::new(PushManagerInner { queue: VecDeque::new(), max_queue })) } }
    pub fn publish(&self, topic: &str, payload: &str) {
        let mut g = self.inner.lock().unwrap();
        if g.queue.len() >= g.max_queue { g.queue.pop_front(); }
        g.queue.push_back(PushMessage { topic: topic.into(), payload: payload.into(), timestamp_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() });
    }
    pub fn drain(&self) -> Vec<PushMessage> { let mut g = self.inner.lock().unwrap(); g.queue.drain(..).collect() }
    pub fn len(&self) -> usize { self.inner.lock().unwrap().queue.len() }
}

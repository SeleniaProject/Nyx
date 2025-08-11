#![forbid(unsafe_code)]
//! Periodic HPKE session rekey manager.
//!
//! Policy:
//! - Rekey if (elapsed_time >= time_interval) OR (packets_since_rekey >= packet_interval).
//! - Grace window keeps previous key valid for decryption until grace expiry or next rekey.
//! - Provides callbacks/hooks so integration layer can send RekeyFrame when initiating.
//!
//! This is a lightweight state machine; it does not perform network I/O itself.

use std::time::{Duration, Instant};
use std::sync::Arc;
use nyx_crypto::noise::SessionKey;
#[cfg(feature="telemetry")]
use nyx_telemetry::{inc_hpke_rekey_grace_used, inc_hpke_rekey_cooldown_suppressed, observe_hpke_key_lifetime};

#[derive(Debug, Clone)]
pub struct RekeyPolicy {
    pub time_interval: Duration,
    pub packet_interval: u64,
    pub grace_period: Duration,
    pub min_cooldown: Duration, // minimum enforced time between successive rekeys (anti-spam)
}

impl Default for RekeyPolicy {
    fn default() -> Self { Self { time_interval: Duration::from_secs(900), packet_interval: 100_000, grace_period: Duration::from_secs(30), min_cooldown: Duration::from_secs(5) } }
}

pub struct HpkeRekeyManager {
    policy: RekeyPolicy,
    current_key: SessionKey,
    previous_key: Option<(SessionKey, Instant)>, // key + grace expiry
    last_rekey: Instant,
    packets_since_rekey: u64,
    grace_notifier: Option<Arc<dyn Fn() + Send + Sync>>, // invoked exactly once when previous key grace ends
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RekeyDecision {
    NoAction,
    Initiate, // caller should generate & send RekeyFrame (outbound)
}

impl std::fmt::Debug for HpkeRekeyManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HpkeRekeyManager")
            .field("policy", &self.policy)
            .field("last_rekey", &self.last_rekey.elapsed())
            .field("packets_since_rekey", &self.packets_since_rekey)
            .field("previous_key_active", &self.previous_key_active())
            .finish()
    }
}

impl HpkeRekeyManager {
    pub fn new(policy: RekeyPolicy, initial_key: SessionKey) -> Self {
        Self { policy, current_key: initial_key, previous_key: None, last_rekey: Instant::now(), packets_since_rekey: 0, grace_notifier: None }
    }

    /// Register a callback invoked when a previous key's grace period ends (and it is purged).
    pub fn set_grace_expiry_notifier(&mut self, f: Arc<dyn Fn() + Send + Sync>) { self.grace_notifier = Some(f); }

    /// Internal: purge expired previous key and fire notifier (once).
    fn maintenance(&mut self) {
        if let Some((_, exp)) = &self.previous_key {
            if Instant::now() > *exp {
                // Drop previous key & notify
                self.previous_key = None;
                if let Some(cb) = &self.grace_notifier { cb(); }
            }
        }
    }

    /// Record that one application packet has been protected with the current key.
    pub fn on_packet_sent(&mut self) -> RekeyDecision {
    self.maintenance();
        self.packets_since_rekey += 1;
        self.evaluate()
    }

    /// Evaluate policy without incrementing counters.
    pub fn evaluate(&self) -> RekeyDecision {
    // (Immutable borrow) – cannot call maintenance here directly, caller should call maintenance() prior if needed.
        let elapsed = self.last_rekey.elapsed();
        if elapsed >= self.policy.time_interval || self.packets_since_rekey >= self.policy.packet_interval {
            if elapsed >= self.policy.min_cooldown {
                RekeyDecision::Initiate
            } else {
                #[cfg(feature="telemetry")]
                inc_hpke_rekey_cooldown_suppressed();
                RekeyDecision::NoAction
            }
        } else { RekeyDecision::NoAction }
    }

    /// Apply newly generated key (after we sealed & sent RekeyFrame). Moves old key into grace window.
    pub fn install_new_key(&mut self, new_key: SessionKey) {
        let now = Instant::now();
        let lifetime = now.duration_since(self.last_rekey);
        #[cfg(feature="telemetry")]
        observe_hpke_key_lifetime(lifetime.as_secs_f64());
        let grace_expiry = now + self.policy.grace_period;
        let old = std::mem::replace(&mut self.current_key, new_key);
        self.previous_key = Some((old, grace_expiry));
        self.last_rekey = now;
        self.packets_since_rekey = 0;
    }

    /// Accept inbound rekey (opened HPKE frame).
    pub fn accept_remote_rekey(&mut self, new_key: SessionKey) {
        self.install_new_key(new_key);
    }

    /// Attempt to decrypt using current or (if within grace) previous key. Caller supplies a closure that tries decryption.
    pub fn try_decrypt<F, T>(&mut self, mut attempt: F) -> Option<T>
    where F: FnMut(&SessionKey) -> Option<T> {
    self.maintenance();
        if let Some(res) = attempt(&self.current_key) { return Some(res); }
    // previous key may have been cleared above
        if let Some((prev, _)) = &self.previous_key {
            let out = attempt(prev);
            if out.is_some() { #[cfg(feature="telemetry")] inc_hpke_rekey_grace_used(); }
            out
        } else { None }
    }

    pub fn current_key(&self) -> &SessionKey { &self.current_key }
    pub fn previous_key_active(&self) -> bool { self.previous_key.as_ref().map(|(_,exp)| Instant::now() <= *exp).unwrap_or(false) }
    pub fn last_rekey_elapsed(&self) -> Duration { self.last_rekey.elapsed() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyx_crypto::noise::SessionKey;

    fn sk(v: u8) -> SessionKey { SessionKey([v;32]) }

    #[test]
    fn decision_logic() {
        let policy = RekeyPolicy { time_interval: Duration::from_millis(50), packet_interval: 10, grace_period: Duration::from_millis(20), min_cooldown: Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, sk(1));
        for _ in 0..9 { assert_eq!(mgr.on_packet_sent(), RekeyDecision::NoAction); }
        assert_eq!(mgr.on_packet_sent(), RekeyDecision::Initiate); // packet threshold
    }

    #[test]
    fn install_and_grace() {
        let policy = RekeyPolicy { time_interval: Duration::from_secs(999), packet_interval: 999_999, grace_period: Duration::from_millis(30), min_cooldown: Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, sk(1));
        mgr.install_new_key(sk(2));
        assert!(mgr.previous_key_active());
        let out = mgr.try_decrypt(|k| if k.0[0]==2 { Some(42) } else { None });
        assert_eq!(out, Some(42));
    }
    #[test]
    fn cooldown_enforced() {
        let policy = RekeyPolicy { time_interval: Duration::from_millis(1), packet_interval: 1, grace_period: Duration::from_millis(10), min_cooldown: Duration::from_millis(50) };
        let mut mgr = HpkeRekeyManager::new(policy, sk(9));
    // First packet reaches packet threshold but min_cooldown prevents immediate rekey.
    assert_eq!(mgr.on_packet_sent(), RekeyDecision::NoAction);
    // Advance artificial time by sleeping beyond min_cooldown so next packet triggers Initiate.
    std::thread::sleep(Duration::from_millis(55));
    assert_eq!(mgr.on_packet_sent(), RekeyDecision::Initiate);
    }
    #[test]
    fn grace_expires() {
        let policy = RekeyPolicy { time_interval: Duration::from_secs(999), packet_interval: 999_999, grace_period: Duration::from_millis(5), min_cooldown: Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, sk(1));
        mgr.install_new_key(sk(2));
        assert!(mgr.previous_key_active());
        std::thread::sleep(Duration::from_millis(7));
        mgr.maintenance();
        assert!(!mgr.previous_key_active());
    }

    #[test]
    fn grace_notifier_fires() {
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
        let flag = Arc::new(AtomicBool::new(false));
        let policy = RekeyPolicy { time_interval: Duration::from_secs(999), packet_interval: 999_999, grace_period: Duration::from_millis(5), min_cooldown: Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, sk(11));
        let f2 = flag.clone();
        mgr.set_grace_expiry_notifier(Arc::new(move || { f2.store(true, Ordering::SeqCst); }));
        mgr.install_new_key(sk(12)); // old key enters grace
        std::thread::sleep(Duration::from_millis(8));
        mgr.maintenance();
        assert!(flag.load(Ordering::SeqCst), "grace expiry notifier did not fire");
    }

    #[test]
    #[cfg(feature="telemetry")]
    fn grace_usage_increments_counter() {
        // Register metrics in default registry so gather() sees them
        let reg = prometheus::default_registry();
        nyx_telemetry::ensure_hpke_rekey_metrics_registered(reg);
        let policy = RekeyPolicy { time_interval: Duration::from_secs(999), packet_interval: 999_999, grace_period: Duration::from_millis(50), min_cooldown: Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, sk(3));
        // Rotate once so old key enters grace
        mgr.install_new_key(sk(4));
        // Closure that only succeeds with previous key (value 3) causing grace path usage
        let _ = mgr.try_decrypt(|k| if k.0[0]==3 { Some(7usize) } else { None });
        let families = prometheus::gather();
        let mut ok=false; for f in families { if f.get_name()=="nyx_hpke_rekey_grace_used_total" { if let Some(m)=f.get_metric().get(0) { if m.get_counter().get_value()>=1.0 { ok=true; } } } }
        assert!(ok, "expected grace used counter to increment");
    }
}

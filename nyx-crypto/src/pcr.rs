//! Post-Compromise Recovery (PCR) automatic rekey scheduler
//!
//! 提供機能:
//! - 一定インターバル or 使用量閾値 (送信ノンス数) による自動セッションキー再生成
//! - 明示トリガ API
//! - 再鍵交換時に旧鍵 zeroize + コールバック通知
//!
//! 実運用では上位層 (stream/transport) が送受信パケット数/時間に応じて `maybe_rekey` を呼ぶ想定。

use crate::{noise::SessionKey, pcr_rekey};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;

#[derive(Debug, Clone)]
pub struct PcrPolicy {
    pub interval: Duration,            // 時間間隔再鍵
    pub max_packets_before_rekey: u64, // 送信ノンス上限
}
impl Default for PcrPolicy {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(900),
            max_packets_before_rekey: 1_000_000,
        }
    }
}

#[derive(Debug)]
pub struct PcrRekeyStats {
    pub performed: u64,
    pub last_at: Option<Instant>,
    pub packet_counter: u64,
}
impl Default for PcrRekeyStats {
    fn default() -> Self {
        Self {
            performed: 0,
            last_at: None,
            packet_counter: 0,
        }
    }
}

pub type RekeyCallback = Arc<dyn Fn(&SessionKey) + Send + Sync + 'static>;

pub struct PcrRekeyManager {
    key: RwLock<SessionKey>,
    policy: RwLock<PcrPolicy>,
    stats: RwLock<PcrRekeyStats>,
    cb: Option<RekeyCallback>,
    started: RwLock<bool>,
}

impl PcrRekeyManager {
    pub fn new(initial_key: SessionKey, cb: Option<RekeyCallback>) -> Arc<Self> {
        Arc::new(Self {
            key: RwLock::new(initial_key),
            policy: RwLock::new(PcrPolicy::default()),
            stats: RwLock::new(PcrRekeyStats::default()),
            cb,
            started: RwLock::new(false),
        })
    }
    pub async fn set_policy(&self, policy: PcrPolicy) {
        *self.policy.write().await = policy;
    }
    pub async fn current_key(&self) -> SessionKey {
        self.key.read().await.clone()
    }
    pub async fn increment_packet_counter(&self, n: u64) {
        self.stats.write().await.packet_counter += n;
    }

    pub async fn maybe_rekey(&self) {
        let policy = self.policy.read().await.clone();
        let stats = self.stats.write().await;
        let need = stats.packet_counter >= policy.max_packets_before_rekey;
        if need {
            drop(stats);
            self.perform_rekey("packet-threshold").await;
        }
    }

    pub async fn perform_rekey(&self, _reason: &str) {
        let mut key_guard = self.key.write().await;
        let mut old = key_guard.clone();
        let new_key = pcr_rekey(&mut old); // old zeroized
        *key_guard = new_key.clone();
        drop(key_guard);
        let mut stats = self.stats.write().await;
        stats.performed += 1;
        stats.last_at = Some(Instant::now());
        stats.packet_counter = 0;
        drop(stats);
        if let Some(cb) = &self.cb {
            cb(&new_key);
        }
        // telemetry gatedログ削除: feature 未定義のためビルド警告抑制
    }

    pub async fn start(self: &Arc<Self>) {
        let mut started = self.started.write().await;
        if *started {
            return;
        }
        *started = true;
        drop(started);
        let me = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                let interval_dur = { me.policy.read().await.interval };
                let mut tick = interval(interval_dur);
                tick.tick().await; // initial schedule
                loop {
                    tick.tick().await;
                    me.perform_rekey("time-interval").await;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_rekey_packet_threshold() {
        let m = PcrRekeyManager::new(SessionKey([1u8; 32]), None);
        m.set_policy(PcrPolicy {
            interval: Duration::from_secs(3600),
            max_packets_before_rekey: 10,
        })
        .await;
        m.increment_packet_counter(5).await;
        m.maybe_rekey().await;
        assert_eq!(m.stats.read().await.performed, 0);
        m.increment_packet_counter(5).await;
        m.maybe_rekey().await;
        assert_eq!(m.stats.read().await.performed, 1);
    }
}

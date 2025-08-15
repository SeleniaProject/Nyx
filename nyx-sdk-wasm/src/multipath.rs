//! WASM-side Multipath control and query API.
//!
//! This module provides a self-contained, browser-safe multipath controller
//! exposing scheduling and statistics management functionality to JavaScript.
//! It mirrors the semantics of the native multipath manager while avoiding
//! OS and threading primitives not available on wasm32-unknown-unknown.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use wasm_bindgen::prelude::*;
// Note: Ordering import removed as unused in current implementation.

/// Identifier type for a path (0-255)
pub type PathId = u8;

/// Default history size for selection entropy estimation
const DEFAULT_HISTORY: usize = 256;

/// Default reorder buffer size used by global/local buffers
const DEFAULT_REORDER_BUFFER: usize = 2048;

/// Default timeout for out-of-order packet expiration in milliseconds
const DEFAULT_REORDER_TIMEOUT_MS: u64 = 200;

/// Configuration for the multipath controller (WASM-safe subset)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipathConfig {
    pub max_paths: usize,
    pub min_paths: usize,
    pub reorder_global: bool,
    pub reorder_buffer_size: usize,
    pub reorder_timeout_ms: u64,
    pub fairness_entropy_floor: f64,
}

impl Default for MultipathConfig {
    fn default() -> Self {
        Self {
            max_paths: 16,
            min_paths: 1,
            reorder_global: false,
            reorder_buffer_size: DEFAULT_REORDER_BUFFER,
            reorder_timeout_ms: DEFAULT_REORDER_TIMEOUT_MS,
            fairness_entropy_floor: 0.7,
        }
    }
}

/// Public JS-facing config wrapper
#[wasm_bindgen]
pub struct MultipathConfigWasm {
    inner: MultipathConfig,
}

#[wasm_bindgen]
impl MultipathConfigWasm {
    /// Create config from JSON string. Unspecified fields use defaults.
    /// Unknown fields are ignored.
    #[wasm_bindgen(constructor)]
    pub fn new(json_config: Option<String>) -> Result<MultipathConfigWasm, JsValue> {
        if let Some(cfg) = json_config {
            let mut base = MultipathConfig::default();
            let incoming: serde_json::Value = serde_json::from_str(&cfg)
                .map_err(|e| JsValue::from_str(&format!("Invalid config JSON: {}", e)))?;
            if let Some(v) = incoming.get("max_paths").and_then(|v| v.as_u64()) {
                base.max_paths = v as usize;
            }
            if let Some(v) = incoming.get("min_paths").and_then(|v| v.as_u64()) {
                base.min_paths = v as usize;
            }
            if let Some(v) = incoming.get("reorder_global").and_then(|v| v.as_bool()) {
                base.reorder_global = v;
            }
            if let Some(v) = incoming.get("reorder_buffer_size").and_then(|v| v.as_u64()) {
                base.reorder_buffer_size = v as usize;
            }
            if let Some(v) = incoming.get("reorder_timeout_ms").and_then(|v| v.as_u64()) {
                base.reorder_timeout_ms = v;
            }
            if let Some(v) = incoming
                .get("fairness_entropy_floor")
                .and_then(|v| v.as_f64())
            {
                base.fairness_entropy_floor = v;
            }
            Ok(MultipathConfigWasm { inner: base })
        } else {
            Ok(MultipathConfigWasm {
                inner: MultipathConfig::default(),
            })
        }
    }

    /// Serialize the config to JSON string
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Per-path dynamic telemetry used for adaptive weighting
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathStats {
    pub latency_ms: f64,
    pub jitter_ms: f64,
    pub loss_rate: f64,
    pub bandwidth_kbps: f64,
    pub success_count: u64,
    pub failure_count: u64,
    pub active: bool,
}

impl PathStats {
    pub fn health_score(&self) -> f64 {
        // Lower latency, lower jitter, lower loss, higher bandwidth -> better.
        // Normalize with soft floors to avoid division by zero.
        let l = (self.latency_ms.max(0.1)).recip();
        let j = (self.jitter_ms.max(0.1)).recip();
        let bw = (self.bandwidth_kbps.max(1.0)).ln_1p();
        let loss_penalty = 1.0 / (1.0 + self.loss_rate.max(0.0));
        let reliability = (self.success_count as f64 + 1.0)
            / (self.success_count as f64 + self.failure_count as f64 + 2.0);
        let active_bonus = if self.active { 1.0 } else { 0.5 };
        let score = l * 0.35 + j * 0.15 + bw * 0.25 + reliability * 0.25;
        score * loss_penalty * active_bonus
    }
}

/// JS-facing stats wrapper
#[wasm_bindgen]
pub struct PathStatsWasm {
    inner: PathStats,
}

#[wasm_bindgen]
impl PathStatsWasm {
    #[wasm_bindgen(constructor)]
    pub fn new(
        latency_ms: f64,
        jitter_ms: f64,
        loss_rate: f64,
        bandwidth_kbps: f64,
        active: bool,
    ) -> PathStatsWasm {
        PathStatsWasm {
            inner: PathStats {
                latency_ms,
                jitter_ms,
                loss_rate,
                bandwidth_kbps,
                success_count: 0,
                failure_count: 0,
                active,
            },
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Internal representation of one WRR path state
#[derive(Debug, Clone)]
struct WrrPath {
    id: PathId,
    base_weight: u32,
    effective_weight: f64,
    current: f64,
    stats: PathStats,
}

/// Smooth Weighted Round Robin scheduler
#[derive(Debug, Default)]
struct SmoothWrr {
    paths: Vec<WrrPath>,
    total_weight: f64,
}

impl SmoothWrr {
    fn add_path(&mut self, id: PathId, weight: u32, stats: PathStats) {
        let w = weight.max(1) as f64;
        self.paths.push(WrrPath {
            id,
            base_weight: weight.max(1),
            effective_weight: w,
            current: 0.0,
            stats,
        });
        self.recompute_total();
    }

    fn remove_path(&mut self, id: PathId) {
        self.paths.retain(|p| p.id != id);
        self.recompute_total();
    }

    fn recompute_total(&mut self) {
        self.total_weight = self.paths.iter().map(|p| p.effective_weight).sum();
        if self.total_weight <= 0.0 {
            self.total_weight = 1.0;
        }
    }

    fn adjust_weights_from_health(&mut self) {
        for p in &mut self.paths {
            // Map health score to multiplier in [0.25, 2.0]
            let h = p.stats.health_score();
            let mult = (h / 1.0).clamp(0.25, 2.0);
            p.effective_weight = (p.base_weight as f64) * mult;
        }
        self.recompute_total();
    }

    fn select(&mut self) -> Option<PathId> {
        if self.paths.is_empty() {
            return None;
        }
        // Smooth WRR selection
        let mut best_idx: usize = 0;
        let mut best_val: f64 = f64::MIN;
        for (i, p) in self.paths.iter_mut().enumerate() {
            p.current += p.effective_weight;
            if p.current > best_val {
                best_val = p.current;
                best_idx = i;
            }
        }
        let total = self.total_weight;
        self.paths[best_idx].current -= total;
        Some(self.paths[best_idx].id)
    }
}

/// Reordering buffer entry
#[derive(Debug, Clone)]
struct BufferedPacket {
    seq: u64,
}

/// Per-path reordering buffer (sequence-number based)
#[derive(Debug)]
struct ReorderBuffer {
    next_expected: u64,
    buf: VecDeque<BufferedPacket>,
    cap: usize,
}

impl ReorderBuffer {
    fn new(cap: usize) -> Self {
        Self {
            next_expected: 0,
            buf: VecDeque::with_capacity(cap),
            cap,
        }
    }

    fn insert(&mut self, seq: u64) {
        if self.buf.len() >= self.cap {
            self.buf.pop_front();
        }
        // Keep sorted by sequence number
        let pos = self
            .buf
            .iter()
            .position(|p| p.seq > seq)
            .unwrap_or(self.buf.len());
        self.buf.insert(pos, BufferedPacket { seq });
    }

    fn pop_contiguous(&mut self) -> Vec<u64> {
        let mut out = Vec::new();
        loop {
            match self.buf.front() {
                Some(front) if front.seq == self.next_expected => {
                    let pkt = self.buf.pop_front().unwrap();
                    out.push(pkt.seq);
                    self.next_expected = self.next_expected.saturating_add(1);
                }
                _ => break,
            }
        }
        out
    }
}

/// Selection result returned to JS
#[wasm_bindgen]
pub struct PathSelectionResult {
    pub path_id: u8,
    pub weight: u32,
}

/// Main controller exposed to JS
#[wasm_bindgen]
pub struct MultipathController {
    cfg: MultipathConfig,
    wrr: SmoothWrr,
    reorder_global: Option<ReorderBuffer>,
    reorder_per_path: HashMap<PathId, ReorderBuffer>,
    selection_history: Vec<PathId>,
    fixed_weights: bool,
}

#[wasm_bindgen]
impl MultipathController {
    /// Create a new controller from a config JSON (optional).
    #[wasm_bindgen(constructor)]
    pub fn new(config: Option<MultipathConfigWasm>) -> MultipathController {
        let cfg = config.map(|c| c.inner).unwrap_or_default();
        let reorder_global = if cfg.reorder_global {
            Some(ReorderBuffer::new(cfg.reorder_buffer_size))
        } else {
            None
        };
        MultipathController {
            cfg,
            wrr: SmoothWrr::default(),
            reorder_global,
            reorder_per_path: HashMap::new(),
            selection_history: Vec::with_capacity(DEFAULT_HISTORY),
            fixed_weights: false,
        }
    }

    /// Add a path with initial weight; stats can be provided as JSON string.
    /// Stats JSON fields: latency_ms, jitter_ms, loss_rate, bandwidth_kbps, active
    pub fn add_path(
        &mut self,
        path_id: u8,
        initial_weight: u32,
        stats_json: Option<String>,
    ) -> Result<(), JsValue> {
        if self.wrr.paths.len() >= self.cfg.max_paths {
            return Err(JsValue::from_str("Maximum number of paths reached"));
        }
        let stats: PathStats = match stats_json {
            Some(s) => serde_json::from_str(&s)
                .map_err(|e| JsValue::from_str(&format!("Invalid stats JSON: {}", e)))?,
            None => PathStats::default(),
        };
        self.wrr.add_path(path_id, initial_weight, stats);
        if !self.cfg.reorder_global {
            self.reorder_per_path
                .entry(path_id)
                .or_insert_with(|| ReorderBuffer::new(self.cfg.reorder_buffer_size));
        }
        Ok(())
    }

    /// Remove a path from the controller.
    pub fn remove_path(&mut self, path_id: u8) -> Result<(), JsValue> {
        if self.wrr.paths.len() <= self.cfg.min_paths {
            return Err(JsValue::from_str("Cannot remove path below min_paths"));
        }
        self.wrr.remove_path(path_id);
        self.reorder_per_path.remove(&path_id);
        Ok(())
    }

    /// Update per-path dynamic statistics and recompute effective weights.
    pub fn update_stats(&mut self, path_id: u8, stats: PathStatsWasm) -> Result<(), JsValue> {
        let mut found = false;
        for p in &mut self.wrr.paths {
            if p.id == path_id {
                p.stats = stats.inner;
                found = true;
                break;
            }
        }
        if !found {
            return Err(JsValue::from_str("Path not found"));
        }
        self.wrr.adjust_weights_from_health();
        Ok(())
    }

    /// Select the next path for sending data using Smooth WRR.
    pub fn select_path(&mut self) -> Option<PathSelectionResult> {
        if self.wrr.paths.is_empty() {
            return None;
        }
        // Refresh weights based on health unless fixed_weights mode is enabled
        if !self.fixed_weights {
            self.wrr.adjust_weights_from_health();
        }
        let id = self.wrr.select()?;
        if self.selection_history.len() >= DEFAULT_HISTORY {
            self.selection_history.remove(0);
        }
        self.selection_history.push(id);
        let weight = self
            .wrr
            .paths
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.effective_weight as u32)
            .unwrap_or(1);
        Some(PathSelectionResult {
            path_id: id,
            weight,
        })
    }

    /// Push an observed packet sequence for the given path, returning any newly contiguous sequences.
    pub fn push_sequence(&mut self, path_id: u8, seq: u64) -> Result<JsValue, JsValue> {
        if self.cfg.reorder_global {
            if let Some(buf) = self.reorder_global.as_mut() {
                buf.insert(seq);
                let out = buf.pop_contiguous();
                return serde_wasm_bindgen::to_value(&out)
                    .map_err(|e| JsValue::from_str(&e.to_string()));
            }
            return Err(JsValue::from_str(
                "Global reordering buffer not initialized",
            ));
        } else {
            let buf = self
                .reorder_per_path
                .get_mut(&path_id)
                .ok_or_else(|| JsValue::from_str("Path reordering buffer missing"))?;
            buf.insert(seq);
            let out = buf.pop_contiguous();
            return serde_wasm_bindgen::to_value(&out)
                .map_err(|e| JsValue::from_str(&e.to_string()));
        }
    }

    /// Get a JSON summary of current paths, weights and stats.
    pub fn get_summary_json(&self) -> String {
        #[derive(Serialize)]
        struct Entry {
            path_id: u8,
            base_weight: u32,
            effective_weight: f64,
            stats: PathStats,
        }
        let mut entries: Vec<Entry> = self
            .wrr
            .paths
            .iter()
            .map(|p| Entry {
                path_id: p.id,
                base_weight: p.base_weight,
                effective_weight: p.effective_weight,
                stats: p.stats.clone(),
            })
            .collect();
        entries.sort_by(|a, b| a.path_id.cmp(&b.path_id));
        serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
    }

    /// Reset selection history and WRR internal counters.
    pub fn reset(&mut self) {
        for p in &mut self.wrr.paths {
            p.current = 0.0;
        }
        self.selection_history.clear();
    }

    /// Enable or disable fixed weight mode (disables health-driven adjustments when true)
    pub fn set_fixed_weights(&mut self, fixed: bool) {
        self.fixed_weights = fixed;
    }

    /// Explicitly recompute weights from the current per-path health metrics
    pub fn recompute_weights(&mut self) {
        self.wrr.adjust_weights_from_health();
    }

    /// Set base weight for a specific path and update its effective weight accordingly
    pub fn set_path_weight(&mut self, path_id: u8, weight: u32) -> Result<(), JsValue> {
        let mut updated = false;
        for p in &mut self.wrr.paths {
            if p.id == path_id {
                p.base_weight = weight.max(1);
                p.effective_weight = p.base_weight as f64; // immediate effect; health may modulate later
                updated = true;
                break;
            }
        }
        if !updated {
            return Err(JsValue::from_str("Path not found"));
        }
        self.wrr.recompute_total();
        Ok(())
    }

    /// Return recent selection history as JSON array
    pub fn get_selection_history_json(&self) -> String {
        serde_json::to_string(&self.selection_history).unwrap_or_else(|_| "[]".to_string())
    }
}

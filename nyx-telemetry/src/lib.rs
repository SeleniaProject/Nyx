#![forbid(unsafe_code)]

#[derive(thiserror::Error, Debug)]
pub enum Error { #[error("telemetry init failed: {0}")] Init(String) }
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exporter { None, Prometheus, Otlp }

#[derive(Debug, Clone)]
pub struct Config { pub exporter: Exporter }

impl Default for Config { fn default() -> Self { Self { exporter: Exporter::None } } }

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

static REGISTRY: Lazy<prometheus::Registry> = Lazy::new(prometheus::Registry::new);
static COUNTERS: Lazy<Mutex<HashMap<String, prometheus::IntCounter>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn init(cfg: &Config) -> Result<()> {
	match cfg.exporter {
		Exporter::None => Ok(()),
		Exporter::Prometheus => Ok(()),
		Exporter::Otlp => Ok(()),
	}
}

pub fn record_counter(name: &str, v: u64) {
	let mut map = COUNTERS.lock().unwrap();
	let ctr = map.entry(name.to_string()).or_insert_with(|| {
		let c = prometheus::IntCounter::new(name, format!("counter {name}")).expect("counter");
		REGISTRY.register(Box::new(c.clone())).ok();
		c
	});
	ctr.inc_by(v);
}

/// Prometheusテキストフォーマットでメトリクスをダンプ
pub fn dump_prometheus() -> String {
	use prometheus::{Encoder, TextEncoder};
	let mf = REGISTRY.gather();
	let enc = TextEncoder::new();
	let mut buf = Vec::new();
	enc.encode(&mf, &mut buf).ok();
	String::from_utf8(buf).unwrap_or_default()
}

#[cfg(test)]
mod tests { use super::*; #[test] fn init_noop() { init(&Config::default()).unwrap(); } }


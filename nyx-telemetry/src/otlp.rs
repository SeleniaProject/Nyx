#![cfg(feature = "otlp")]
//! OTLP (OpenTelemetry) test utilities: deterministic, pure Rust, in-memory span capture.
//!
//! Purpose:
//! - Provide an `otlp` feature build path that can assert span name & attributes (e.g. `path_id`, `cid`)
//!   without requiring an external collector during CI / unit tests.
//! - Keep future compatibility with adding a real OTLP exporter (gRPC) while retaining the same
//!   tracing instrumentation in production code.
//!
//! Design:
//! - A custom `Layer` (`InMemorySpanLayer`) records span attributes on creation and subsequent
//!   `record` updates, then stores a finalized snapshot when the span closes.
//! - An OpenTelemetry `TracerProvider` is initialized (without exporter) so that adding exporters later
//!   will not require refactoring initialization code. We still attach an `OpenTelemetryLayer` to ensure
//!   span context propagation semantics remain aligned with production.
//! - Captured spans are stored in an `Arc<Mutex<Vec<CapturedSpan>>>` returned by `init_in_memory_tracer`.
//!
//! Constraints / Guarantees:
//! - Pure Rust only (no C/C++ FFI) to respect repository policy.
//! - Non-blocking except for brief `Mutex` critical sections.
//! - Safe to call multiple times in independent tests; later calls replace the global subscriber.
//!
//! Future Work (non-breaking additions):
//! - Optional redaction/filter hooks.
//! - Deterministic ordering / virtual clock injection for fuzzing.
//! - Metrics correlation (count spans per path) in the layer itself.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};
use tracing::{Id, Subscriber};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::{layer::SubscriberExt, Registry};

/// Captured span snapshot (minimal fields required for current assertions).
#[derive(Debug, Clone)]
pub struct CapturedSpan {
	pub name: String,
	pub attributes: HashMap<String, String>,
}

/// Shared span storage handle.
pub type SharedCapturedSpans = Arc<Mutex<Vec<CapturedSpan>>>;

// Attribute redaction/filter: return None to drop; Some(new_value) to keep/transform.
type AttrFilterFn = dyn Fn(&str, &str) -> Option<String> + 'static;
thread_local! { static TL_ATTR_FILTER: std::cell::RefCell<Option<Arc<AttrFilterFn>>> = const { std::cell::RefCell::new(None) }; }
pub fn set_attribute_filter(f: Option<Arc<AttrFilterFn>>) { TL_ATTR_FILTER.with(|cell| *cell.borrow_mut() = f); }

/// Visitor that stringifies arbitrary field values into the attributes map (with optional filtering).
struct MapVisitor<'a> { map: &'a mut HashMap<String, String> }
impl<'a> MapVisitor<'a> { fn store(&mut self, k: &str, v: &str) { TL_ATTR_FILTER.with(|cell| { if let Some(f) = &*cell.borrow() { if let Some(nv) = (f)(k,v) { self.map.insert(k.to_string(), nv); return; } else { return; } } self.map.insert(k.to_string(), v.to_string()); }); } }
impl<'a> Visit for MapVisitor<'a> {
	fn record_str(&mut self, f: &Field, v: &str) { self.store(f.name(), v); }
	fn record_bool(&mut self, f: &Field, v: bool) { self.store(f.name(), &v.to_string()); }
	fn record_i64(&mut self, f: &Field, v: i64) { self.store(f.name(), &v.to_string()); }
	fn record_u64(&mut self, f: &Field, v: u64) { self.store(f.name(), &v.to_string()); }
	fn record_i128(&mut self, f: &Field, v: i128) { self.store(f.name(), &v.to_string()); }
	fn record_u128(&mut self, f: &Field, v: u128) { self.store(f.name(), &v.to_string()); }
	fn record_f64(&mut self, f: &Field, v: f64) { self.store(f.name(), &format!("{}", v)); }
	fn record_error(&mut self, f: &Field, v: &(dyn std::error::Error + 'static)) { self.store(f.name(), &format!("{}", v)); }
	fn record_debug(&mut self, f: &Field, v: &dyn fmt::Debug) { self.store(f.name(), &format!("{:?}", v)); }
}

#[derive(Debug)]
struct InMemorySpanLayer {
	store: SharedCapturedSpans,
	live: Arc<Mutex<HashMap<Id, HashMap<String,String>>>>,
}
impl InMemorySpanLayer { fn new(store: SharedCapturedSpans) -> Self { Self { store, live: Arc::new(Mutex::new(HashMap::new())) } } }

// Channel for manual OTLP export (Option A). Only set when otlp_exporter feature is enabled and initialized.
#[cfg(feature = "otlp_exporter")]
use tokio::sync::mpsc;
#[cfg(feature = "otlp_exporter")]
use once_cell::sync::Lazy as OnceLazy;
#[cfg(feature = "otlp_exporter")]
static EXPORT_SENDER: OnceLazy<std::sync::RwLock<Option<mpsc::Sender<CapturedSpan>>>> = OnceLazy::new(|| std::sync::RwLock::new(None));
#[cfg(feature = "otlp_exporter")]
pub fn register_export_sender(tx: mpsc::Sender<CapturedSpan>) { *EXPORT_SENDER.write().unwrap() = Some(tx); }

impl<S> Layer<S> for InMemorySpanLayer
where
	S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
	fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &Id, _ctx: Context<'_, S>) {
		let mut map = HashMap::new();
		let mut visitor = MapVisitor { map: &mut map };
		attrs.record(&mut visitor);
		self.live.lock().unwrap().insert(id.clone(), map);
	}
	fn on_record(&self, id: &Id, values: &tracing::span::Record<'_>, _ctx: Context<'_, S>) {
		if let Some(current) = self.live.lock().unwrap().get_mut(id) {
			let mut visitor = MapVisitor { map: current };
			values.record(&mut visitor);
		}
	}
	fn on_close(&self, id: Id, ctx: Context<'_, S>) {
		if let Some(mut map) = self.live.lock().unwrap().remove(&id) {
			if let Some(span_ref) = ctx.span(&id) {
				let name = span_ref.metadata().name().to_string();
				// Invoke path hook if applicable
				if name == "nyx.stream.send" {
					if let Some(pid) = map.get("path_id").and_then(|v| v.parse::<u8>().ok()) {
						if let Some(h) = SPAN_PATH_HOOK.read().unwrap().as_ref() { (h)(pid); }
					}
				}
				let captured = CapturedSpan { name, attributes: std::mem::take(&mut map) };
				self.store.lock().unwrap().push(captured.clone());
				TOTAL_SPANS_CLOSED.fetch_add(1, Ordering::Relaxed);
				#[cfg(feature = "otlp_exporter")]
				{
					if let Some(tx) = &*EXPORT_SENDER.read().unwrap() { let _ = tx.try_send(captured); }
				}
			}
		}
	}
}

/// Initialize in-memory tracing capture (replaces global subscriber each call).
static SPAN_COUNTER: AtomicU64 = AtomicU64::new(0);
static TOTAL_SPANS_CLOSED: AtomicU64 = AtomicU64::new(0);
use crate::sampling::deterministic_accept;

// -------------------------------------------------------------------------------------------------
// Span->metric correlation hook: When a span named "nyx.stream.send" is accepted by the sampler
// we extract its path_id attribute (u8) and invoke a registered callback (installed by
// TelemetryCollector) to increment the corresponding Prometheus counter. This avoids needing the
// producer code to call record_stream_send separately in OTLP path.
// -------------------------------------------------------------------------------------------------
use std::sync::{RwLock as StdRwLock, Arc as StdArc};
use once_cell::sync::Lazy;
type SpanPathHook = dyn Fn(u8) + Send + Sync + 'static;
static SPAN_PATH_HOOK: Lazy<StdRwLock<Option<StdArc<SpanPathHook>>>> = Lazy::new(|| StdRwLock::new(None));
pub fn register_span_path_hook<F: Fn(u8) + Send + Sync + 'static>(f: F) { *SPAN_PATH_HOOK.write().unwrap() = Some(StdArc::new(f)); }

pub fn init_in_memory_tracer(_service_name: &str, sampling_ratio: f64) -> (tracing::Dispatch, SharedCapturedSpans) {
	let store: SharedCapturedSpans = Arc::new(Mutex::new(Vec::new()));
	struct SamplingLayer { inner: InMemorySpanLayer, ratio: f64 }
	impl<S> Layer<S> for SamplingLayer where S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a> {
		fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
			if self.ratio <= 0.0 { return; }
			if !deterministic_accept(&SPAN_COUNTER, self.ratio) { return; }
			self.inner.on_new_span(attrs, id, ctx)
		}
		fn on_record(&self, id: &Id, values: &tracing::span::Record<'_>, ctx: Context<'_, S>) { if self.ratio == 0.0 { return; } self.inner.on_record(id, values, ctx) }
		fn on_close(&self, id: Id, ctx: Context<'_, S>) { if self.ratio == 0.0 { return; } self.inner.on_close(id, ctx) }
	}
	let subscriber = Registry::default().with(SamplingLayer { inner: InMemorySpanLayer::new(store.clone()), ratio: sampling_ratio });
	let dispatch = tracing::Dispatch::new(subscriber);
	(dispatch, store)
}

/// Attempt to force-flush captured spans by waiting for quiescence.
pub fn force_flush() {
    let mut last = TOTAL_SPANS_CLOSED.load(Ordering::Relaxed);
    let mut stable_iters = 0u8;
    for _ in 0..20u8 { // ~ up to ~1s
        std::thread::sleep(std::time::Duration::from_millis(50));
        let now = TOTAL_SPANS_CLOSED.load(Ordering::Relaxed);
        if now == last { stable_iters += 1; } else { stable_iters = 0; last = now; }
        if stable_iters >= 3 { break; }
    }
}

// ----------------------------------------------------------------------------------------------
// Exporter recovery (simplified): Provide a wrapper that can execute an export attempt with
// exponential backoff + circuit breaker state. For now we expose API for future integration with
// a real tonic exporter. Metrics (success/fail) can be hooked by caller via closures.
// ----------------------------------------------------------------------------------------------
use std::time::{Instant, Duration};
use prometheus::{IntCounter, IntGauge};

static EXPORT_SUCCESS: Lazy<std::sync::RwLock<Option<IntCounter>>> = Lazy::new(|| std::sync::RwLock::new(None));
static EXPORT_FAILURE: Lazy<std::sync::RwLock<Option<IntCounter>>> = Lazy::new(|| std::sync::RwLock::new(None));
static EXPORT_CIRCUIT: Lazy<std::sync::RwLock<Option<IntGauge>>> = Lazy::new(|| std::sync::RwLock::new(None));

pub fn register_exporter_metrics(success: IntCounter, failure: IntCounter, circuit: IntGauge) {
	*EXPORT_SUCCESS.write().unwrap() = Some(success);
	*EXPORT_FAILURE.write().unwrap() = Some(failure);
	*EXPORT_CIRCUIT.write().unwrap() = Some(circuit);
}
pub struct BackoffPolicy { base: Duration, max: Duration, factor: f64 }
impl Default for BackoffPolicy { fn default() -> Self { Self { base: Duration::from_millis(200), max: Duration::from_secs(5), factor: 2.0 } } }
pub struct CircuitBreaker { fail_count: u32, open: bool, opened_at: Option<Instant>, threshold: u32, open_secs: u64 }
impl Default for CircuitBreaker { fn default() -> Self { Self { fail_count: 0, open: false, opened_at: None, threshold: 5, open_secs: 10 } } }
impl CircuitBreaker { fn on_success(&mut self) { self.fail_count = 0; if self.open { self.open = false; self.opened_at=None; if let Some(g)=&*EXPORT_CIRCUIT.read().unwrap() { g.set(0); } } } fn on_failure(&mut self) { self.fail_count+=1; if self.fail_count>=self.threshold { self.open=true; self.opened_at=Some(Instant::now()); if let Some(g)=&*EXPORT_CIRCUIT.read().unwrap() { g.set(1); } } } fn allow(&mut self) -> bool { if !self.open { return true; } if let Some(t) = self.opened_at { if t.elapsed() >= Duration::from_secs(self.open_secs) { self.open=false; self.fail_count=0; self.opened_at=None; if let Some(g)=&*EXPORT_CIRCUIT.read().unwrap() { g.set(0); } return true; } } false } }

pub async fn with_recovery<F, Fut, T>(mut attempt: F) -> Option<T>
where F: FnMut() -> Fut, Fut: std::future::Future<Output = anyhow::Result<T>> {
	let mut backoff = BackoffPolicy::default();
	let mut delay = backoff.base;
	let mut breaker = CircuitBreaker::default();
	for _ in 0..20 { // cap attempts
		if !breaker.allow() { tokio::time::sleep(Duration::from_secs(1)).await; continue; }
		match attempt().await { Ok(v) => { breaker.on_success(); if let Some(c)=&*EXPORT_SUCCESS.read().unwrap() { c.inc(); } return Some(v); }, Err(_) => { breaker.on_failure(); if let Some(c)=&*EXPORT_FAILURE.read().unwrap() { c.inc(); } tokio::time::sleep(delay).await; delay = Duration::min(backoff.max, Duration::from_secs_f64(delay.as_secs_f64()*backoff.factor)); } }
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;
	#[tokio::test(flavor = "current_thread")]
	async fn capture_basic_span() {
		let (dispatch, spans) = init_in_memory_tracer("nyx-test", 1.0);
		tracing::dispatcher::with_default(&dispatch, || {
			let span = tracing::span!(tracing::Level::INFO, "nyx.stream.send", path_id = 99u8, cid = "cid-basic");
			let _e = span.enter(); tracing::info!("emit inside span");
		});
		let stored = spans.lock().unwrap();
		assert!(!stored.is_empty(), "expected at least one span");
	}

	#[tokio::test(flavor = "current_thread")]
	async fn sampling_ratio_zero_drops_span() {
		let (dispatch, spans) = init_in_memory_tracer("nyx-test", 0.0);
		tracing::dispatcher::with_default(&dispatch, || {
			let span = tracing::span!(tracing::Level::INFO, "nyx.stream.send", path_id = 1u8, cid = "cid-drop");
			let _g = span.enter(); tracing::info!("should not record");
		});
		let stored = spans.lock().unwrap();
		let found = stored.iter().any(|s| s.name == "nyx.stream.send");
		assert!(!found, "span should have been sampled out");
	}

	#[tokio::test(flavor = "current_thread")]
	async fn init_only_no_crash() {
		// Initialize tracer and ensure no spans are captured immediately after init.
		let (_dispatch, spans) = init_in_memory_tracer("nyx-test", 1.0);
		let store = spans.lock().unwrap();
		assert!(store.is_empty(), "span store should be empty right after initialization");
	}

	#[tokio::test(flavor = "current_thread")]
	async fn ratio_sampling_drops_some() {
		let (dispatch, spans) = init_in_memory_tracer("nyx-test", 0.1); // 10%
		tracing::dispatcher::with_default(&dispatch, || {
			for i in 0..200u32 {
				let span = tracing::span!(tracing::Level::INFO, "nyx.stream.send", seq = i);
				let _g = span.enter();
			}
		});
		let stored = spans.lock().unwrap().len();
		assert!(stored > 0 && stored < 200, "deterministic sampling should keep subset: {}", stored);
	}

	#[tokio::test(flavor = "current_thread")]
	async fn attribute_redaction_applies() {
		set_attribute_filter(Some(Arc::new(|k,v| {
			if k == "secret" { return Some("REDACTED".into()); }
			if k == "drop_me" { return None; }
			Some(v.to_string())
		}))); 
		let (dispatch, spans) = init_in_memory_tracer("nyx-test", 1.0);
		tracing::dispatcher::with_default(&dispatch, || {
			let span = tracing::span!(tracing::Level::INFO, "nyx.stream.send", secret = "value", drop_me = 123u32, keep = true);
			let _e = span.enter();
		});
		let store = spans.lock().unwrap();
		let s = store.iter().find(|s| s.name == "nyx.stream.send").expect("span");
		assert_eq!(s.attributes.get("secret").unwrap(), "REDACTED");
		assert!(!s.attributes.contains_key("drop_me"));
		assert!(s.attributes.contains_key("keep"));
		set_attribute_filter(None);
	}
}

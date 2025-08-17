//! OpenTelemetry integration (feature = "otlp").

#[cfg(feature = "otlp")]
pub fn init_tracing(_service_name: Option<String>) -> anyhow::Result<()> {
	// To keep compilation stable across crate versions, we currently install a
	// minimal fmt subscriber when the feature is enabled. Wiring a full OTLP
	// pipeline can be added later against the exact crate versions in use.
	if tracing::dispatcher::has_been_set() {
		// Already initialized elsewhere; do nothing.
		return Ok(());
	}
	let subscriber = tracing_subscriber::fmt().with_target(false).finish();
	tracing::subscriber::set_global_default(subscriber)?;
	Ok(())
}


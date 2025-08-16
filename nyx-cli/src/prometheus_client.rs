#![forbid(unsafe_code)]

use anyhow::Result;

/// Fetch Prometheus text exposition format over HTTP (no TLS).
pub async fn scrape_text(url: String) -> Result<String> {
	// ureq is blocking; run in a blocking task to avoid stalling the runtime
	let body = tokio::task::spawn_blocking(move || {
		let resp = ureq::get(&url).call()?;
		let text = resp.into_string()?;
		anyhow::Ok(text)
	})
	.await
	.map_err(|e| anyhow::anyhow!("join error: {e}"))??;
	Ok(body)
}


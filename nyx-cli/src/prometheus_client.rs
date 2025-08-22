#![forbid(unsafe_code)]

use anyhow::Result;

/// Fetch Prometheus text exposition format over HTTP (no TLS).
pub async fn scrape_text(url: String) -> Result<String> {
    if url.starts_with("https://") {
        anyhow::bail!("https is not supported by nyx-cli prometheus-get (TLS disabled)");
    }
    // ureq is blocking; run in a blocking task to avoid stalling the runtime
    let body = tokio::task::spawn_blocking(move || {
        // Add a conservative timeout to avoid indefinite hangs.
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(8))
            .build();
        let resp = agent.get(&url).call()?;
        let text = resp.into_string()?;
        anyhow::Ok(text)
    })
    .await
    .map_err(|e| anyhow::anyhow!("join error: {e}"))??;
    Ok(body)
}

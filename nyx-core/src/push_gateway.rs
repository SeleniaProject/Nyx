use crate::performance::RateLimiter;
use crate::push::PushProvider;
use std::sync::Arc;

pub struct PushGateway<P> {
	provider: Arc<P>,
	limiter: std::sync::Mutex<RateLimiter>,
}

impl<P: PushProvider + 'static> PushGateway<P> {
	pub fn new(provider: Arc<P>, per_sec: f64) -> Self { Self { provider, limiter: std::sync::Mutex::new(RateLimiter::new(1.0, per_sec)) } }
	pub async fn send(&self, token: &str, title: &str, body: &str) -> anyhow::Result<bool> {
		let mut l = self.limiter.lock().unwrap();
		if !l.allow() { return Ok(false); }
		drop(l);
		self.provider.send(token, title, body).await?;
		Ok(true)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::push::LoggingPush;
	use std::time::Duration;

	#[tokio::test]
	async fn gateway_rate_limits() {
		let gw = PushGateway::new(Arc::new(LoggingPush), 1000.0);
	assert!(gw.send("t", "a", "b").await.unwrap());
	// rate-limited immediately after capacity consumed
	assert!(!gw.send("t", "a", "b").await.unwrap());
		// allow after wait
		tokio::time::sleep(Duration::from_millis(10)).await;
	}
}

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
		{
			// Be resilient to mutex poisoning; recover inner limiter to avoid panic_s in library code
			let mut l = match self.limiter.lock() { Ok(g) => g, Err(p) => p.into_inner() };
			if !l.allow() { return Ok(false); }
		}
		self.provider.send(token, title, body).await?;
		Ok(true)
	}
}

#[cfg(test)]
mod test_s {
	use super::*;
	use crate::push::LoggingPush;
	use std::time::Duration;

	#[tokio::test]
	async fn gateway_rate_limit_s() {
		let _gw = PushGateway::new(Arc::new(LoggingPush), 1000.0);
	assert!(gw.send("t", "a", "b").await.unwrap());
	// rate-limited immediately after capacity consumed
	assert!(!gw.send("t", "a", "b").await.unwrap());
		// allow after wait
		tokio::time::sleep(Duration::from_milli_s(10)).await;
	}

	#[tokio::test]
	async fn gateway_mutex_poison_recovery() {
		struct NoopPush;
		#[async_trait::async_trait]
		impl crate::push::PushProvider for NoopPush {
			async fn send(&self, _token: &str, _title: &str, _body: &str) -> anyhow::Result<()> { Ok(()) }
		}
		let _gw = Arc::new(PushGateway::new(Arc::new(NoopPush), 1000.0));
		// Intentionally poison the mutex by panicking while holding the lock
		let _gwc = gw.clone();
		let _handle = std::thread::spawn(move || {
			let __g = gwc.limiter.lock()?;
			return Err("intentional poison".into());
		});
		let __ = handle.join();

		// After poisoning, send should not panic and should return either true/false cleanly
		let _r = gw.send("t", "a", "b").await;
		assert!(r.is_ok());
	}
}

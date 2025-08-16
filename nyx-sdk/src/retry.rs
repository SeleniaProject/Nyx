#![forbid(unsafe_code)]

use std::future::Future;

pub async fn retry<F, Fut, T, E>(mut f: F, mut next_delay_ms: impl FnMut(u32) -> u64, max_attempts: u32) -> Result<T, E>
where
	F: FnMut() -> Fut,
	Fut: Future<Output = Result<T, E>>,
{
	let mut attempt = 0u32;
	loop {
		match f().await {
			Ok(v) => return Ok(v),
			Err(e) => {
				attempt += 1;
				if attempt >= max_attempts { return Err(e); }
				let ms = next_delay_ms(attempt);
				tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
			}
		}
	}
}


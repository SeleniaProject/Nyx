#![forbid(unsafe_code)]

use std::future::Future;

pub async fn retry<F, Fut, T, E>(
    mut f: F,
    mut next_delay_m_s: impl FnMut(u32) -> u64,
    max_attempt_s: u32,
) -> Result<T, E>
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
                if attempt >= max_attempt_s {
                    return Err(e);
                }
                let m_s = next_delay_m_s(attempt);
                tokio::time::sleep(std::time::Duration::from_millis(m_s)).await;
            }
        }
    }
}

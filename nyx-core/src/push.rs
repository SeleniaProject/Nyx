use async_trait::async_trait;

#[async_trait]
pub trait PushProvider: Send + Sync {
    async fn send(&self, token: &str, title: &str, body: &str) -> anyhow::Result<()>;
}

pub struct LoggingPush;

#[async_trait]
impl PushProvider for LoggingPush {
    async fn send(&self, token: &str, title: &str, body: &str) -> anyhow::Result<()> {
        tracing::info!(target: "push", %token, %title, %body, "push notification (mock)");
        Ok(())
    }
}

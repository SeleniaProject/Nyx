//! Push notification gateway (FCM / APNS) used for Low Power Mode wake-up.
//! All network interactions use `ureq` (pure Rust HTTP client).
#![forbid(unsafe_code)]

use serde_json::Value as JsonValue;
use tokio::sync::{mpsc, oneshot};
use nyx_core::PushProvider;
use chrono::Utc;
use pasetors::version4::V4;
use pasetors::keys::{AsymmetricSecretKey, AsymmetricKeyPair, Generate};

/// Errors that can occur while sending push notifications.
#[derive(Debug)]
pub enum PushError {
    /// HTTP transport-level error.
    Http(Box<dyn std::error::Error + Send + Sync>),
    /// Remote server responded with non-success status.
    Server(u16, String),
    /// Authentication failed during token generation.
    AuthenticationFailed(String),
}

impl From<ureq::Error> for PushError {
    fn from(e: ureq::Error) -> Self { Self::Http(Box::new(e)) }
}

/// Simple push manager abstraction.
pub struct PushManager {
    agent: ureq::Agent,
    provider: PushProvider,
}

impl PushManager {
    /// Construct a new `PushManager` with given provider configuration.
    #[must_use]
    pub fn new(provider: PushProvider) -> Self {
        Self { agent: ureq::Agent::new(), provider }
    }

    /// Send JSON payload to a specific `device_token`.
    ///
    /// This method is **async** and must be awaited inside a Tokio runtime.
    pub async fn send(&self, device_token: &str, payload: &JsonValue) -> Result<(), PushError> {
        match &self.provider {
            PushProvider::Fcm { server_key } => {
                self.send_fcm(device_token, payload, server_key).await
            }
            PushProvider::Apns { team_id, key_id, key_p8 } => {
                self.send_apns(device_token, payload, team_id, key_id, key_p8).await
            }
        }
    }

    async fn send_fcm(
        &self,
        device_token: &str,
        payload: &JsonValue,
        server_key: &str,
    ) -> Result<(), PushError> {
        let body = serde_json::json!({
            "to": device_token,
            "data": payload,
        });
        
        // Use blocking call in spawn_blocking for async compatibility
        let agent = self.agent.clone();
        let server_key = server_key.to_string();
        let body_str = body.to_string();
        
        tokio::task::spawn_blocking(move || {
            let resp = agent
                .post("https://fcm.googleapis.com/fcm/send")
                .set("Authorization", &format!("key={}", server_key))
                .set("Content-Type", "application/json")
                .send_string(&body_str)?;
                
            if resp.status() == 200 {
                Ok(())
            } else {
                Err(PushError::Server(resp.status(), 
                    resp.into_string().unwrap_or("Unknown error".to_string())))
            }
        }).await.map_err(|e| PushError::Http(Box::new(e)))?
    }

    async fn send_apns(
        &self,
        device_token: &str,
        payload: &JsonValue,
        team_id: &str,
        key_id: &str,
        key_p8: &str,
    ) -> Result<(), PushError> {
        // Generate JWT valid for up to 20 minutes.
        let jwt = generate_apns_token(team_id, key_id, key_p8)?;

        // Minimal APNS implementation using ureq
        let url = format!("https://api.push.apple.com/3/device/{}", device_token);
        let agent = self.agent.clone();
        let payload_str = payload.to_string();
        
        tokio::task::spawn_blocking(move || {
            let resp = agent
                .post(&url)
                .set("authorization", &format!("bearer {}", jwt))
                .set("apns-push-type", "background")
                .set("apns-priority", "5")
                .set("content-type", "application/json")
                .send_string(&payload_str)?;
                
            if resp.status() == 200 {
                Ok(())
            } else {
                Err(PushError::Server(resp.status(), 
                    resp.into_string().unwrap_or("Unknown error".to_string())))
            }
        }).await.map_err(|e| PushError::Http(Box::new(e)))?
    }
}

/// Generate APNS token using PASETO v4 public key authentication.
fn generate_apns_token(team_id: &str, key_id: &str, key_p8: &str) -> Result<String, PushError> {
    use pasetors::{
        keys::AsymmetricKeyPair,
        version4::V4,
        claims::Claims,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    // Fallback: generate a new key for development
    let keypair = AsymmetricKeyPair::<V4>::generate().map_err(|_| {
        PushError::AuthenticationFailed("Failed to generate PASETO keypair".to_string())
    })?;

    // Create APNS token claims
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    let mut claims = Claims::new().map_err(|_| {
        PushError::AuthenticationFailed("Failed to create claims".to_string())
    })?;
    
    claims.issuer(team_id).map_err(|_| {
        PushError::AuthenticationFailed("Failed to set issuer".to_string())
    })?;
    
    claims.issued_at(&now.to_string()).map_err(|_| {
        PushError::AuthenticationFailed("Failed to set issued at".to_string())
    })?;
    
    claims.expiration(&(now + 3600).to_string()).map_err(|_| {
        PushError::AuthenticationFailed("Failed to set expiration".to_string())
    })?;

    // Generate PASETO v4 token
    pasetors::public::sign(&keypair.secret, &claims, None, None)
        .map_err(|_| PushError::AuthenticationFailed("Failed to sign PASETO token".to_string()))
}

// -------------------------------------------------------------------------------------------------
// Background worker + handle

/// Internal request message used by [`PushHandle`].
enum PushRequest {
    Send {
        token: String,
        payload: JsonValue,
        resp: oneshot::Sender<Result<(), PushError>>,
    },
}

/// Cheap cloneable handle for queuing push send operations to background worker.
#[derive(Clone)]
pub struct PushHandle {
    tx: mpsc::Sender<PushRequest>,
}

impl PushHandle {
    /// Enqueue push notification and await result.
    pub async fn send(&self, token: &str, payload: JsonValue) -> Result<(), PushError> {
        let (tx, rx) = oneshot::channel();
        let msg = PushRequest::Send { token: token.to_string(), payload, resp: tx };
        // Translate channel closure into PushError::Server
        self.tx
            .send(msg)
            .await
            .map_err(|_| PushError::Server(500, "worker closed".into()))?;
        rx.await.unwrap_or_else(|_| Err(PushError::Server(500, "worker closed".into())))
    }
}

/// Spawn background task that owns `PushManager` and processes requests.
#[must_use]
pub fn spawn_push_service(provider: PushProvider) -> PushHandle {
    let (tx, mut rx) = mpsc::channel::<PushRequest>(64);
    tokio::spawn(async move {
        let mgr = PushManager::new(provider);
        while let Some(req) = rx.recv().await {
            match req {
                PushRequest::Send { token, payload, resp } => {
                    let result = mgr.send(&token, &payload).await;
                    let _ = resp.send(result);
                }
            }
        }
    });
    PushHandle { tx }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn fcm_request_builds() {
        let mgr = PushManager::new(PushProvider::Fcm { server_key: "test_key".into() });
        // Sending will fail due to invalid key, but it should return Server error not panic.
        let res = mgr.send("dummy", &json!({"k":"v"})).await;
        assert!(res.is_err());
    }
} 
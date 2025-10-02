//! Session Management REST API
//!
//! Provides HTTP/JSON API for session status queries instead of gRPC
//! (to avoid C/C++ dependencies from tonic/ring/openssl stack).
//!
//! ## Endpoints
//! - GET  /api/v1/sessions - List all sessions
//! - GET  /api/v1/sessions/:id - Get session status
//! - POST /api/v1/sessions/:id/close - Close a session
//!
//! ## Design rationale
//! - Pure Rust HTTP stack (axum + hyper)
//! - JSON serialization (serde_json)
//! - No C/C++ dependencies (avoids tonic/protobuf/ring/openssl)
//! - Compatible with existing session_manager module

#![forbid(unsafe_code)]

use crate::session_manager::SessionManager;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// Session status response (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusResponse {
    pub session_id: u32,
    pub role: String,
    pub state: String,
    pub age_ms: u64,
    pub idle_time_ms: u64,
    pub has_traffic_keys: bool,
    pub metrics: SessionMetricsResponse,
}

/// Session metrics response (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetricsResponse {
    pub bytes_tx: u64,
    pub bytes_rx: u64,
    pub frames_tx: u64,
    pub frames_rx: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handshake_duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub established_at_ms: Option<u64>,
}

/// List sessions query parameters
#[derive(Debug, Clone, Deserialize)]
pub struct ListSessionsQuery {
    /// Filter by state (e.g., "established", "idle")
    #[serde(rename = "state")]
    pub state_filter: Option<String>,
    /// Filter by role (e.g., "client", "server")
    #[serde(rename = "role")]
    pub role_filter: Option<String>,
}

/// List sessions response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionStatusResponse>,
    pub total_count: usize,
}

/// API error response
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.code.as_str() {
            "NOT_FOUND" => StatusCode::NOT_FOUND,
            "INVALID_REQUEST" => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        
        (status, Json(self)).into_response()
    }
}

/// App state shared across handlers
#[derive(Clone)]
pub struct ApiState {
    pub session_manager: Arc<SessionManager>,
}

/// Creates the session API router
///
/// # Arguments
/// * `session_manager` - The SessionManager instance to query
///
/// # Returns
/// An axum Router with all session API routes configured
pub fn create_session_router(session_manager: Arc<SessionManager>) -> Router {
    let state = ApiState { session_manager };
    
    Router::new()
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions/:id", get(get_session_status))
        .route("/api/v1/sessions/:id/close", post(close_session))
        .with_state(state)
}

/// GET /api/v1/sessions - List all sessions
///
/// Optional query parameters:
/// - `state`: Filter by state (e.g., "established", "idle")
/// - `role`: Filter by role (e.g., "client", "server")
async fn list_sessions(
    State(_state): State<ApiState>,
    Query(query): Query<ListSessionsQuery>,
) -> Result<Json<ListSessionsResponse>, ErrorResponse> {
    info!("GET /api/v1/sessions (state={:?}, role={:?})", 
          query.state_filter, query.role_filter);
    
    // TODO: Implement actual filtering once SessionManager supports listing
    // For now, return empty list (implementation will be completed in integration phase)
    let response = ListSessionsResponse {
        sessions: vec![],
        total_count: 0,
    };
    
    Ok(Json(response))
}

/// GET /api/v1/sessions/:id - Get session status
async fn get_session_status(
    State(state): State<ApiState>,
    Path(session_id): Path<u32>,
) -> Result<Json<SessionStatusResponse>, ErrorResponse> {
    info!("GET /api/v1/sessions/{}", session_id);
    
    let status = state.session_manager.get_session_status(session_id).await;
    
    match status {
        Some(s) => {
            let response = SessionStatusResponse {
                session_id,
                role: format!("{:?}", s.role),
                state: format!("{:?}", s.state),
                age_ms: s.age.as_millis() as u64,
                idle_time_ms: s.idle_time.as_millis() as u64,
                has_traffic_keys: s.has_traffic_keys,
                metrics: SessionMetricsResponse {
                    bytes_tx: s.metrics.bytes_tx,
                    bytes_rx: s.metrics.bytes_rx,
                    frames_tx: s.metrics.frames_tx,
                    frames_rx: s.metrics.frames_rx,
                    handshake_duration_ms: s.metrics.handshake_duration.map(|d| d.as_millis() as u64),
                    // Note: established_at is Instant (not SystemTime), so we return None for now
                    // Proper implementation would require storing SystemTime in SessionMetrics
                    established_at_ms: None,
                },
            };
            Ok(Json(response))
        }
        None => Err(ErrorResponse {
            error: format!("Session {} not found", session_id),
            code: "NOT_FOUND".to_string(),
        }),
    }
}

/// POST /api/v1/sessions/:id/close - Close session
async fn close_session(
    State(state): State<ApiState>,
    Path(session_id): Path<u32>,
) -> Result<StatusCode, ErrorResponse> {
    info!("POST /api/v1/sessions/{}/close", session_id);
    
    match state.session_manager.close_session(session_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to close session {}: {:?}", session_id, e),
            code: "INTERNAL_ERROR".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_manager::SessionManagerConfig;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // For oneshot()
    
    #[tokio::test]
    async fn test_get_session_status_not_found() {
        let manager = Arc::new(SessionManager::new(SessionManagerConfig::default()));
        let app = create_session_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/sessions/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    
    #[tokio::test]
    async fn test_get_session_status_found() {
        let manager = Arc::new(SessionManager::new(SessionManagerConfig::default()));
        
        // Create a client session
        let session_id = manager.create_client_session().await.unwrap();
        
        let app = create_session_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }
    
    #[tokio::test]
    async fn test_close_session() {
        let manager = Arc::new(SessionManager::new(SessionManagerConfig::default()));
        
        // Create a client session
        let session_id = manager.create_client_session().await.unwrap();
        
        let app = create_session_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{}/close", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
    
    #[tokio::test]
    async fn test_list_sessions_empty() {
        let manager = Arc::new(SessionManager::new(SessionManagerConfig::default()));
        let app = create_session_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/sessions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        use http_body_util::BodyExt;
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let list: ListSessionsResponse = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(list.sessions.len(), 0);
        assert_eq!(list.total_count, 0);
    }
    
    #[tokio::test]
    async fn test_session_api_integration() {
        let manager = Arc::new(SessionManager::new(SessionManagerConfig::default()));
        
        // Create a client session
        let session_id = manager.create_client_session().await.unwrap();
        
        let app = create_session_router(manager.clone());
        
        // Get status
        let response = app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{}", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        // Close session
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/sessions/{}/close", session_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
}

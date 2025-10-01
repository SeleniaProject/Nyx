//! Connection Manager REST API
//!
//! Provides HTTP/JSON API for connection management:
//! - GET /api/v1/connections - List all connections
//! - GET /api/v1/connections/:id - Get connection status
//! - POST /api/v1/connections/:id/close - Close connection
//!
//! Uses pure Rust HTTP stack (axum) to avoid C/C++ dependencies.

#![forbid(unsafe_code)]

use crate::connection_manager::ConnectionManager;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

/// Connection status response (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStatusResponse {
    pub id: u32,
    pub age_ms: u64,
    pub idle_time_ms: u64,
    pub cwnd: usize,
    pub btlbw_bps: u64,
    pub srtt_ms: u64,
    pub min_rtt_ms: u64,
    pub max_rtt_ms: u64,
    pub bytes_tx: u64,
    pub bytes_rx: u64,
    pub packets_tx: u64,
    pub packets_rx: u64,
    pub retx_queue_len: usize,
}

/// List connections response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListConnectionsResponse {
    pub connections: Vec<ConnectionStatusResponse>,
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

/// App state
#[derive(Clone)]
pub struct ApiState {
    pub connection_manager: Arc<ConnectionManager>,
}

/// Creates the connection API router
pub fn create_connection_router(connection_manager: Arc<ConnectionManager>) -> Router {
    let state = ApiState { connection_manager };
    
    Router::new()
        .route("/api/v1/connections", get(list_connections))
        .route("/api/v1/connections/:id", get(get_connection_status))
        .route("/api/v1/connections/:id/close", post(close_connection))
        .with_state(state)
}

/// GET /api/v1/connections - List all connections
async fn list_connections(
    State(state): State<ApiState>,
) -> Result<Json<ListConnectionsResponse>, ErrorResponse> {
    info!("GET /api/v1/connections");
    
    let conn_ids = state.connection_manager.list_connections().await;
    let mut connections = Vec::new();

    for conn_id in &conn_ids {
        if let Some(status) = state.connection_manager.get_connection_status(*conn_id).await {
            connections.push(ConnectionStatusResponse {
                id: status.id,
                age_ms: status.age.as_millis() as u64,
                idle_time_ms: status.idle_time.as_millis() as u64,
                cwnd: status.cwnd,
                btlbw_bps: status.btlbw,
                srtt_ms: status.srtt.as_millis() as u64,
                min_rtt_ms: status.min_rtt.as_millis() as u64,
                max_rtt_ms: status.max_rtt.as_millis() as u64,
                bytes_tx: status.bytes_tx,
                bytes_rx: status.bytes_rx,
                packets_tx: status.packets_tx,
                packets_rx: status.packets_rx,
                retx_queue_len: status.retx_queue_len,
            });
        }
    }

    let response = ListConnectionsResponse {
        total_count: connections.len(),
        connections,
    };

    Ok(Json(response))
}

/// GET /api/v1/connections/:id - Get connection status
async fn get_connection_status(
    State(state): State<ApiState>,
    Path(conn_id): Path<u32>,
) -> Result<Json<ConnectionStatusResponse>, ErrorResponse> {
    info!("GET /api/v1/connections/{}", conn_id);
    
    let status = state.connection_manager.get_connection_status(conn_id).await;
    
    match status {
        Some(s) => {
            let response = ConnectionStatusResponse {
                id: s.id,
                age_ms: s.age.as_millis() as u64,
                idle_time_ms: s.idle_time.as_millis() as u64,
                cwnd: s.cwnd,
                btlbw_bps: s.btlbw,
                srtt_ms: s.srtt.as_millis() as u64,
                min_rtt_ms: s.min_rtt.as_millis() as u64,
                max_rtt_ms: s.max_rtt.as_millis() as u64,
                bytes_tx: s.bytes_tx,
                bytes_rx: s.bytes_rx,
                packets_tx: s.packets_tx,
                packets_rx: s.packets_rx,
                retx_queue_len: s.retx_queue_len,
            };
            Ok(Json(response))
        }
        None => Err(ErrorResponse {
            error: format!("Connection {} not found", conn_id),
            code: "NOT_FOUND".to_string(),
        }),
    }
}

/// POST /api/v1/connections/:id/close - Close connection
async fn close_connection(
    State(state): State<ApiState>,
    Path(conn_id): Path<u32>,
) -> Result<StatusCode, ErrorResponse> {
    info!("POST /api/v1/connections/{}/close", conn_id);
    
    match state.connection_manager.close_connection(conn_id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err(ErrorResponse {
            error: format!("Failed to close connection {}: {:?}", conn_id, e),
            code: "INTERNAL_ERROR".to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection_manager::ConnectionManagerConfig;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_get_connection_status_not_found() {
        let manager = Arc::new(ConnectionManager::new(ConnectionManagerConfig::default()));
        let app = create_connection_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/connections/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_connection_status_found() {
        let manager = Arc::new(ConnectionManager::new(ConnectionManagerConfig::default()));
        
        // Create a connection
        let conn_id = manager.create_connection().await.unwrap();
        
        let app = create_connection_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/connections/{}", conn_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_close_connection() {
        let manager = Arc::new(ConnectionManager::new(ConnectionManagerConfig::default()));
        
        // Create a connection
        let conn_id = manager.create_connection().await.unwrap();
        
        let app = create_connection_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/connections/{}/close", conn_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_list_connections_empty() {
        let manager = Arc::new(ConnectionManager::new(ConnectionManagerConfig::default()));
        let app = create_connection_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/connections")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        use http_body_util::BodyExt;
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let list: ListConnectionsResponse = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(list.connections.len(), 0);
        assert_eq!(list.total_count, 0);
    }

    #[tokio::test]
    async fn test_list_connections_with_data() {
        let manager = Arc::new(ConnectionManager::new(ConnectionManagerConfig::default()));
        
        // Create connections
        let conn1 = manager.create_connection().await.unwrap();
        let conn2 = manager.create_connection().await.unwrap();
        
        let app = create_connection_router(manager);
        
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/connections")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        
        assert_eq!(response.status(), StatusCode::OK);
        
        use http_body_util::BodyExt;
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let list: ListConnectionsResponse = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(list.total_count, 2);
        assert_eq!(list.connections.len(), 2);
        
        let ids: Vec<u32> = list.connections.iter().map(|c| c.id).collect();
        assert!(ids.contains(&conn1));
        assert!(ids.contains(&conn2));
    }
}

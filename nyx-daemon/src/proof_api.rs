//! REST API for RSA Accumulator Proof Distribution
//!
//! Provides HTTP endpoints for proof retrieval and verification.
//!
//! # Endpoints
//! - GET /proofs/{batch_id} - Get proof for specific batch
//! - GET /proofs - List all available batch IDs
//! - POST /proofs/verify - Verify a proof

use crate::proof_distributor::{BatchProof, ProofDistributor, ProofError, VerificationResult};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

/// API error response
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for ProofApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ProofApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ProofApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(ErrorResponse { error: message });
        (status, body).into_response()
    }
}

/// API-specific error wrapper
#[derive(Debug)]
enum ProofApiError {
    NotFound(String),
    InternalError(String),
}

impl From<ProofError> for ProofApiError {
    fn from(err: ProofError) -> Self {
        match err {
            ProofError::ProofNotFound(batch_id) => {
                ProofApiError::NotFound(format!("Proof not found for batch {}", batch_id))
            }
            _ => ProofApiError::InternalError(err.to_string()),
        }
    }
}

/// List response for batch IDs
#[derive(Debug, Serialize)]
struct ListResponse {
    batch_ids: Vec<u64>,
    count: usize,
}

/// Create router for proof API
pub fn create_proof_api(distributor: Arc<ProofDistributor>) -> Router {
    Router::new()
        .route("/proofs/:batch_id", get(get_proof))
        .route("/proofs", get(list_proofs))
        .route("/proofs/verify", post(verify_proof))
        .with_state(distributor)
}

/// GET /proofs/{batch_id} - Get proof for specific batch
async fn get_proof(
    State(distributor): State<Arc<ProofDistributor>>,
    Path(batch_id): Path<u64>,
) -> Result<Json<BatchProof>, ProofApiError> {
    info!("API request: GET /proofs/{}", batch_id);

    let proof = distributor.get_proof(batch_id).await?;
    distributor.record_proof_served().await;

    Ok(Json(proof))
}

/// GET /proofs - List all available batch IDs
async fn list_proofs(
    State(distributor): State<Arc<ProofDistributor>>,
) -> Result<Json<ListResponse>, ProofApiError> {
    info!("API request: GET /proofs");

    let batch_ids = distributor.list_batch_ids().await;
    let count = batch_ids.len();

    Ok(Json(ListResponse { batch_ids, count }))
}

/// POST /proofs/verify - Verify a proof
async fn verify_proof(
    State(distributor): State<Arc<ProofDistributor>>,
    Json(proof): Json<BatchProof>,
) -> Result<Json<VerificationResult>, ProofApiError> {
    info!("API request: POST /proofs/verify for batch {}", proof.batch_id);

    let result = distributor.verify_proof(&proof).await;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use nyx_mix::accumulator::{Accumulator, AccumulatorConfig};
    use crate::proof_distributor::ProofDistributorConfig;
    use tokio::sync::RwLock;
    use tower::ServiceExt;

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_get_proof_endpoint() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = Arc::new(ProofDistributor::new(config, accumulator.clone()));

        // Generate test proof
        let elements = vec![b"test_element".to_vec()];
        distributor.generate_proof(1, &elements).await.unwrap();

        let app = create_proof_api(distributor);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/proofs/1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_proof_not_found() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = Arc::new(ProofDistributor::new(config, accumulator));

        let app = create_proof_api(distributor);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/proofs/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_list_proofs_endpoint() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = Arc::new(ProofDistributor::new(config, accumulator.clone()));

        // Generate multiple proofs
        for batch_id in 1..=3 {
            let elements = vec![format!("element{}", batch_id).into_bytes()];
            distributor.generate_proof(batch_id, &elements).await.unwrap();
        }

        let app = create_proof_api(distributor);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/proofs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}

#[cfg(test)]
mod tests;

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use pokeplanner_core::{AppError, HealthResponse};
use pokeplanner_service::PokePlannerService;
use pokeplanner_storage::JsonFileStorage;
use serde_json::json;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

type AppState = Arc<PokePlannerService>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let storage = Arc::new(
        JsonFileStorage::new("data/jobs".into())
            .await
            .expect("Failed to initialize storage"),
    );
    let service = Arc::new(PokePlannerService::new(storage));

    let app = create_router(service);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind");
    tracing::info!("REST API listening on 0.0.0.0:3000");
    axum::serve(listener, app).await.expect("Server error");
}

pub fn create_router(service: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/jobs", post(submit_job))
        .route("/jobs", get(list_jobs))
        .route("/jobs/{id}", get(get_job))
        .with_state(service)
}

async fn health(State(service): State<AppState>) -> Json<HealthResponse> {
    Json(service.health())
}

async fn submit_job(State(service): State<AppState>) -> impl IntoResponse {
    match service.submit_job().await {
        Ok(job_id) => (StatusCode::ACCEPTED, Json(json!({ "job_id": job_id }))),
        Err(e) => error_response(e),
    }
}

async fn get_job(State(service): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let job_id = match Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(json!({ "error": "Invalid job ID" }))),
    };
    match service.get_job(&job_id).await {
        Ok(job) => (StatusCode::OK, Json(json!(job))),
        Err(e) => error_response(e),
    }
}

async fn list_jobs(State(service): State<AppState>) -> impl IntoResponse {
    match service.list_jobs().await {
        Ok(jobs) => (StatusCode::OK, Json(json!({ "jobs": jobs }))),
        Err(e) => error_response(e),
    }
}

fn error_response(e: AppError) -> (StatusCode, Json<serde_json::Value>) {
    let (status, msg) = match &e {
        AppError::NotFound(_) | AppError::JobNotFound(_) => (StatusCode::NOT_FOUND, e.to_string()),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    (status, Json(json!({ "error": msg })))
}

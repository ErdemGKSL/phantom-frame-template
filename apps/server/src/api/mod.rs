use std::sync::atomic::Ordering;

use axum::{Extension, Json, Router, routing::get};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::AppState;

pub fn api_router() -> Router {
    Router::new()
        .route("/api/counter", get(get_counter))
        .route("/api/increment", get(increment_counter))
}

async fn get_counter(Extension(state): Extension<Arc<AppState>>) -> Json<Value> {
    let value = state.counter.load(Ordering::Relaxed);
    Json(json!({ "value": value }))
}

async fn increment_counter(Extension(state): Extension<Arc<AppState>>) -> Json<Value> {
    let value = state.counter.fetch_add(1, Ordering::Relaxed) + 1;

    // Re-fetch the pre-generated pages from the upstream so the cache reflects
    // the updated counter value immediately.
    if let Err(e) = state.cache_handle.refresh_snapshot("/").await {
        tracing::warn!("Failed to refresh snapshot '/': {}", e);
    }
    if let Err(e) = state.cache_handle.refresh_snapshot("/__data.json").await {
        tracing::warn!("Failed to refresh snapshot '/__data.json': {}", e);
    }

    Json(json!({ "value": value }))
}

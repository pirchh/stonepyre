use axum::{routing::get, Router};

use crate::{api::handlers, state::AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/v1/snapshot", get(handlers::snapshot))
}
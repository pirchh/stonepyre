use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};

use crate::state::AppState;
use super::{handlers, middleware::require_bearer_auth};

pub fn routes(state: AppState) -> Router<AppState> {
    // -----------------------------
    // Public (no auth)
    // -----------------------------
    let public_v1 = Router::new()
        .route("/auth/register", post(handlers::auth::register))
        .route("/auth/login", post(handlers::auth::login));

    // -----------------------------
    // Protected (bearer required)
    // -----------------------------
    let protected_v1 = Router::new()
        .route("/auth/logout", post(handlers::auth::logout))
        .route("/auth/account", delete(handlers::auth::delete_account))

        // WS (game)
        .route("/game/ws", get(handlers::ws::game_ws))

        // Back-compat routes
        .route("/characters", get(handlers::characters::list_slots))
        .route("/characters", post(handlers::characters::create))
        .route("/characters/:id", delete(handlers::characters::delete))

        // Preferred routes (clearer for client)
        .route("/game/characters", get(handlers::characters::list_slots))
        .route("/game/characters", post(handlers::characters::create))
        .route("/game/characters/:id", delete(handlers::characters::delete))
        .route(
            "/game/characters/:id/active-session",
            get(handlers::characters::active_session),
        )

        // Apply auth middleware to ALL above
        .layer(middleware::from_fn_with_state(state.clone(), require_bearer_auth));

    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/v1/market/clock", get(handlers::market::clock))
        .nest("/v1", public_v1)
        .nest("/v1", protected_v1)
}

use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderMap},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub account_id: Uuid,
    pub token: String,
}

/// Middleware: requires Authorization: Bearer <token>
/// - verifies token via `auth.verify_session(token)`
/// - inserts AuthContext into request extensions
pub async fn require_bearer_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let token = extract_bearer(&headers).ok_or(ApiError::Unauthorized)?;

    let account_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT auth.verify_session($1::text)
        "#,
    )
    .bind(&token)
    .fetch_one(&state.db)
    .await?; // uses From<sqlx::Error> for ApiError

    let account_id = account_id.ok_or(ApiError::Unauthorized)?;

    req.extensions_mut().insert(AuthContext {
        account_id,
        token,
    });

    Ok(next.run(req).await)
}

/// Allow handlers to declare `ctx: AuthContext` directly.
/// This pulls the context out of request extensions (set by the middleware).
#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or(ApiError::Unauthorized)
    }
}

fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let raw = raw.trim();

    let prefix = "Bearer ";
    if !raw.starts_with(prefix) {
        return None;
    }

    let token = raw[prefix.len()..].trim();
    if token.is_empty() {
        return None;
    }

    Some(token.to_string())
}
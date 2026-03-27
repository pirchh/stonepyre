use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::ApiError, state::AppState};
use crate::http::middleware::AuthContext;

// Password hashing cost (db function handles the actual hashing)
const DEFAULT_PW_COST: i32 = 12;
// Session TTL seconds (7 days)
const SESSION_TTL_SECS: i32 = 60 * 60 * 24 * 7;

#[derive(Debug, Deserialize)]
pub struct RegisterReq {
    pub email: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResp {
    pub token: String,
    pub account_id: Uuid,
}

fn header_str(headers: &HeaderMap, key: &'static str) -> String {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

/// Return best-effort client IP as Option<String>.
/// - if missing or empty -> None
fn best_effort_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn new_token() -> String {
    let mut b = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterReq>,
) -> Result<impl IntoResponse, ApiError> {
    if req.email.trim().is_empty() || req.password.is_empty() || req.display_name.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "email, password, and display_name are required".to_string(),
        ));
    }

    let token = new_token();
    let user_agent = header_str(&headers, "user-agent");
    let ip = best_effort_ip(&headers);

    let password_hash: String = sqlx::query_scalar(
        r#"
        SELECT auth.hash_password($1::text, $2::int)
        "#,
    )
    .bind(&req.password)
    .bind(DEFAULT_PW_COST)
    .fetch_one(&state.db)
    .await?;

    // Insert account (nice duplicate-email message)
    let account_id_res: Result<Uuid, sqlx::Error> = sqlx::query_scalar(
        r#"
        INSERT INTO auth.accounts (email, password_hash, display_name)
        VALUES ($1::text, $2::text, $3::text)
        RETURNING account_id
        "#,
    )
    .bind(req.email.trim())
    .bind(password_hash)
    .bind(req.display_name.trim())
    .fetch_one(&state.db)
    .await;

    let account_id = match account_id_res {
        Ok(id) => id,
        Err(e) => {
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.code().as_deref() == Some("23505") {
                    return Err(ApiError::BadRequest("email already registered".to_string()));
                }
            }
            return Err(ApiError::from(e));
        }
    };

    // ✅ Cast the 5th arg to inet (matches auth.new_session signature)
    let _session_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT auth.new_session($1::uuid, $2::text, $3::int, $4::text, $5::inet)
        "#,
    )
    .bind(account_id)
    .bind(&token)
    .bind(SESSION_TTL_SECS)
    .bind(user_agent)
    .bind(ip) // Option<String>
    .fetch_one(&state.db)
    .await?;

    Ok(Json(AuthResp { token, account_id }))
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginReq>,
) -> Result<impl IntoResponse, ApiError> {
    if req.email.trim().is_empty() || req.password.is_empty() {
        return Err(ApiError::BadRequest("email and password are required".to_string()));
    }

    let account_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT auth.login($1::text, $2::text)
        "#,
    )
    .bind(req.email.trim())
    .bind(&req.password)
    .fetch_one(&state.db)
    .await?;

    let account_id = account_id.ok_or(ApiError::Unauthorized)?;

    let token = new_token();
    let user_agent = header_str(&headers, "user-agent");
    let ip = best_effort_ip(&headers);

    // ✅ Cast the 5th arg to inet
    let _session_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT auth.new_session($1::uuid, $2::text, $3::int, $4::text, $5::inet)
        "#,
    )
    .bind(account_id)
    .bind(&token)
    .bind(SESSION_TTL_SECS)
    .bind(user_agent)
    .bind(ip) // Option<String>
    .fetch_one(&state.db)
    .await?;

    Ok(Json(AuthResp { token, account_id }))
}

pub async fn logout(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, ApiError> {
    sqlx::query(r#"SELECT auth.revoke_session($1::text)"#)
        .bind(&ctx.token)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE your own account (dev/admin convenience).
/// This is intentionally "self-delete" only for now.
pub async fn delete_account(
    State(state): State<AppState>,
    ctx: AuthContext,
) -> Result<impl IntoResponse, ApiError> {
    let res = sqlx::query(r#"DELETE FROM auth.accounts WHERE account_id = $1::uuid"#)
        .bind(ctx.account_id)
        .execute(&state.db)
        .await?;

    if res.rows_affected() != 1 {
        return Err(ApiError::Internal("account delete failed".to_string()));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
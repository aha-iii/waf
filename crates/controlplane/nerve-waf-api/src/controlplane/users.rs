//! Console 用户管理 API（仅 admin）。

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, put},
    Json, Router,
};
use nerve_waf_auth::AuthMethod;
use nerve_waf_store::ConsoleUser;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::AppState;

pub fn users_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/me", get(get_me))
        .route("/api/users/{id}", put(update_user).delete(delete_user))
}

#[derive(Serialize)]
struct UserJson {
    id: String,
    username: String,
    role: String,
    enabled: bool,
    display_name: Option<String>,
    tenant_id: String,
    created_at: String,
    updated_at: String,
}

impl From<ConsoleUser> for UserJson {
    fn from(u: ConsoleUser) -> Self {
        Self {
            id: u.id,
            username: u.username,
            role: u.role,
            enabled: u.enabled,
            display_name: u.display_name,
            tenant_id: u.tenant_id,
            created_at: u.created_at.to_rfc3339(),
            updated_at: u.updated_at.to_rfc3339(),
        }
    }
}

fn bearer_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(str::trim)
}

fn api_key_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
}

async fn require_authenticated(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(AuthMethod, String, String), StatusCode> {
    let api_key = api_key_from_headers(headers);
    let bearer = bearer_from_headers(headers);
    let method = state.auth.authenticate(api_key, bearer).await;
    if method == AuthMethod::None {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let role = state.auth.resolve_role(method, api_key, bearer);
    let username = match method {
        AuthMethod::Jwt => bearer
            .and_then(|t| state.auth.jwt_identity(t))
            .map(|(u, _)| u)
            .unwrap_or_else(|| "jwt".into()),
        AuthMethod::ApiKey => api_key.unwrap_or("api-key").into(),
        AuthMethod::None => "anonymous".into(),
    };
    Ok((method, username, role))
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(String, String), StatusCode> {
    let (_, username, role) = require_authenticated(state, headers).await?;
    if role != nerve_waf_store::ROLE_ADMIN {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok((username, role))
}

async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let store = state.users_store.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let users = store.list().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let items: Vec<UserJson> = users.into_iter().map(UserJson::from).collect();
    Ok(Json(json!({ "total": items.len(), "users": items })))
}

async fn get_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    let (_, username, role) = require_authenticated(&state, &headers).await?;
    Ok(Json(json!({ "username": username, "role": role })))
}

#[derive(Deserialize)]
struct CreateUserBody {
    username: String,
    password: String,
    role: String,
    #[serde(default)]
    display_name: Option<String>,
}

async fn create_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CreateUserBody>,
) -> Result<Json<Value>, StatusCode> {
    let (operator, _) = require_admin(&state, &headers).await?;
    let store = state.users_store.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let user = store
        .create(
            &body.username,
            &body.password,
            &body.role,
            body.display_name.as_deref(),
            &operator,
        )
        .await
        .map_err(map_users_err)?;
    Ok(Json(json!({ "user": UserJson::from(user) })))
}

#[derive(Deserialize)]
struct UpdateUserBody {
    role: Option<String>,
    password: Option<String>,
    enabled: Option<bool>,
    display_name: Option<String>,
    #[serde(default)]
    clear_display_name: bool,
}

async fn update_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<UpdateUserBody>,
) -> Result<Json<Value>, StatusCode> {
    let (operator, _) = require_admin(&state, &headers).await?;
    let store = state.users_store.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    let display_name = if body.clear_display_name {
        Some(None)
    } else if let Some(ref name) = body.display_name {
        Some(Some(name.as_str()))
    } else {
        None
    };
    let user = store
        .update(
            &id,
            body.role.as_deref(),
            body.password.as_deref(),
            body.enabled,
            display_name,
            &operator,
        )
        .await
        .map_err(map_users_err)?;
    Ok(Json(json!({ "user": UserJson::from(user) })))
}

async fn delete_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let (operator, _) = require_admin(&state, &headers).await?;
    let store = state.users_store.as_ref().ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    store
        .delete(&id, &operator)
        .await
        .map_err(map_users_err)?;
    Ok(Json(json!({ "ok": true })))
}

fn map_users_err(err: nerve_waf_store::UsersError) -> StatusCode {
    match err {
        nerve_waf_store::UsersError::NotFound => StatusCode::NOT_FOUND,
        nerve_waf_store::UsersError::Conflict(_) | nerve_waf_store::UsersError::Validation(_) => {
            StatusCode::BAD_REQUEST
        }
        nerve_waf_store::UsersError::Db(_) | nerve_waf_store::UsersError::Crypto(_) => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

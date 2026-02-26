use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};

use super::{
    middleware::AdminState,
    types::{
        AddCredentialRequest, ApiKeyListResponse, ApiStatsResponse, CreateApiKeyRequest,
        CreateApiKeyResponse, LoginRequest, LoginResponse, RequestLogResponse,
        SetApiKeyDisabledRequest, SetApiKeyRoutingRequest, SetDisabledRequest,
        SetLoadBalancingModeRequest, SetPriorityRequest, SuccessResponse,
    },
};

pub async fn login(
    State(state): State<AdminState>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    if !state.verify_login(&payload.username, &payload.password) {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(super::types::AdminErrorResponse::authentication_error()),
        )
            .into_response();
    }

    let session = state.sessions.create_session(&payload.username);
    Json(LoginResponse {
        success: true,
        token: session.token,
        expires_at: session.expires_at,
    })
    .into_response()
}

pub async fn get_all_credentials(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_all_credentials())
}

pub async fn set_credential_disabled(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetDisabledRequest>,
) -> impl IntoResponse {
    match state.service.set_disabled(id, payload.disabled) {
        Ok(_) => Json(SuccessResponse::new("更新成功")).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn set_credential_priority(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
    Json(payload): Json<SetPriorityRequest>,
) -> impl IntoResponse {
    match state.service.set_priority(id, payload.priority) {
        Ok(_) => Json(SuccessResponse::new("更新成功")).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn reset_failure_count(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.reset_and_enable(id) {
        Ok(_) => Json(SuccessResponse::new("重置成功")).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn get_credential_balance(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.get_balance(id).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn add_credential(
    State(state): State<AdminState>,
    Json(payload): Json<AddCredentialRequest>,
) -> impl IntoResponse {
    match state.service.add_credential(payload).await {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn delete_credential(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.delete_credential(id) {
        Ok(_) => Json(SuccessResponse::new("删除成功")).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn get_load_balancing_mode(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_load_balancing_mode())
}

pub async fn set_load_balancing_mode(
    State(state): State<AdminState>,
    Json(payload): Json<SetLoadBalancingModeRequest>,
) -> impl IntoResponse {
    match state.service.set_load_balancing_mode(payload) {
        Ok(response) => Json(response).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn list_api_keys(State(state): State<AdminState>) -> impl IntoResponse {
    Json(ApiKeyListResponse {
        keys: state.service.list_api_keys(),
    })
}

pub async fn create_api_key(
    State(state): State<AdminState>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> impl IntoResponse {
    match state
        .service
        .create_api_key(payload.name, payload.routing_mode, payload.credential_id)
    {
        Ok(key) => Json(CreateApiKeyResponse {
            success: true,
            id: key.id,
            name: key.name,
            key_preview: if key.key.len() > 8 {
                format!("{}****{}", &key.key[..4], &key.key[key.key.len() - 4..])
            } else {
                "********".to_string()
            },
            key: key.key,
            routing_mode: key.routing_mode,
            credential_id: key.credential_id,
        })
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(super::types::AdminErrorResponse::invalid_request(
                e.to_string(),
            )),
        )
            .into_response(),
    }
}

pub async fn set_api_key_routing(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(payload): Json<SetApiKeyRoutingRequest>,
) -> impl IntoResponse {
    match state
        .service
        .set_api_key_routing(&id, payload.routing_mode, payload.credential_id)
    {
        Ok(_) => Json(SuccessResponse::new("更新成功")).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(super::types::AdminErrorResponse::invalid_request(
                e.to_string(),
            )),
        )
            .into_response(),
    }
}

pub async fn set_api_key_disabled(
    State(state): State<AdminState>,
    Path(id): Path<String>,
    Json(payload): Json<SetApiKeyDisabledRequest>,
) -> impl IntoResponse {
    match state.service.set_api_key_enabled(&id, !payload.disabled) {
        Ok(_) => Json(SuccessResponse::new("更新成功")).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(super::types::AdminErrorResponse::invalid_request(
                e.to_string(),
            )),
        )
            .into_response(),
    }
}

pub async fn delete_api_key(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.service.delete_api_key(&id) {
        Ok(_) => Json(SuccessResponse::new("删除成功")).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(super::types::AdminErrorResponse::invalid_request(
                e.to_string(),
            )),
        )
            .into_response(),
    }
}

pub async fn get_api_stats(State(state): State<AdminState>) -> impl IntoResponse {
    Json(ApiStatsResponse {
        overview: state.service.api_key_overview(),
    })
}

pub async fn export_credentials(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.export_credentials())
}

pub async fn export_credential(
    State(state): State<AdminState>,
    Path(id): Path<u64>,
) -> impl IntoResponse {
    match state.service.export_credential(id) {
        Ok(cred) => Json(cred).into_response(),
        Err(e) => (e.status_code(), Json(e.into_response())).into_response(),
    }
}

pub async fn get_total_balance(State(state): State<AdminState>) -> impl IntoResponse {
    Json(state.service.get_total_balance().await)
}

#[derive(Debug, serde::Deserialize)]
pub struct LogQuery {
    pub since_id: Option<String>,
}

pub async fn get_request_logs(
    State(state): State<AdminState>,
    Query(query): Query<LogQuery>,
) -> impl IntoResponse {
    let entries = state.service.get_request_logs(query.since_id.as_deref());
    Json(RequestLogResponse { entries })
}

#[derive(Debug, serde::Deserialize)]
pub struct SetLogEnabledRequest {
    pub enabled: bool,
}

pub async fn set_log_enabled(
    State(state): State<AdminState>,
    Json(payload): Json<SetLogEnabledRequest>,
) -> impl IntoResponse {
    state.service.set_log_enabled(payload.enabled);
    Json(SuccessResponse::new(if payload.enabled { "日志已开启" } else { "日志已关闭" }))
}

pub async fn get_log_enabled(State(state): State<AdminState>) -> impl IntoResponse {
    Json(serde_json::json!({ "enabled": state.service.is_log_enabled() }))
}

pub async fn get_sticky_status(State(state): State<AdminState>) -> impl IntoResponse {
    match state.service.get_sticky_status() {
        Some(resp) => Json(serde_json::to_value(resp).unwrap()).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(super::types::AdminErrorResponse::not_found("Sticky tracker not configured")),
        )
            .into_response(),
    }
}

pub async fn get_sticky_streams(State(state): State<AdminState>) -> impl IntoResponse {
    match state.service.get_sticky_streams() {
        Some(resp) => Json(serde_json::to_value(resp).unwrap()).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(super::types::AdminErrorResponse::not_found("Sticky tracker not configured")),
        )
            .into_response(),
    }
}

pub async fn get_sticky_stats(State(state): State<AdminState>) -> impl IntoResponse {
    match state.service.get_sticky_stats() {
        Some(resp) => Json(serde_json::to_value(resp).unwrap()).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(super::types::AdminErrorResponse::not_found("Sticky tracker not configured")),
        )
            .into_response(),
    }
}

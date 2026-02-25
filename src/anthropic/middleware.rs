//! Anthropic API middleware

use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};

use crate::apikeys::{ApiKeyManager, AuthenticatedApiKey};
use crate::common::auth;
use crate::kiro::provider::KiroProvider;
use crate::request_log::RequestLog;

use super::types::ErrorResponse;

#[derive(Clone)]
pub struct AppState {
    pub api_keys: Arc<ApiKeyManager>,
    pub kiro_provider: Option<Arc<KiroProvider>>,
    pub profile_arn: Option<String>,
    pub request_log: Option<Arc<RequestLog>>,
}

impl AppState {
    pub fn new(api_keys: Arc<ApiKeyManager>) -> Self {
        Self {
            api_keys,
            kiro_provider: None,
            profile_arn: None,
            request_log: None,
        }
    }

    pub fn with_kiro_provider(mut self, provider: KiroProvider) -> Self {
        self.kiro_provider = Some(Arc::new(provider));
        self
    }

    pub fn with_profile_arn(mut self, arn: impl Into<String>) -> Self {
        self.profile_arn = Some(arn.into());
        self
    }

    pub fn with_request_log(mut self, log: Arc<RequestLog>) -> Self {
        self.request_log = Some(log);
        self
    }
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let Some(key) = auth::extract_api_key(&request) else {
        let error = ErrorResponse::authentication_error();
        return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
    };

    let Some(authed) = state.api_keys.authenticate(&key) else {
        let error = ErrorResponse::authentication_error();
        return (StatusCode::UNAUTHORIZED, Json(error)).into_response();
    };

    request
        .extensions_mut()
        .insert::<AuthenticatedApiKey>(authed);
    next.run(request).await
}

pub fn cors_layer() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{Any, CorsLayer};

    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

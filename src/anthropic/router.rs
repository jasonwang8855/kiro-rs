//! Anthropic API router

use std::sync::Arc;

use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};

use crate::apikeys::ApiKeyManager;
use crate::kiro::provider::KiroProvider;

use super::{
    handlers::{count_tokens, get_models, post_messages, post_messages_cc},
    middleware::{AppState, auth_middleware, cors_layer},
};

const MAX_BODY_SIZE: usize = 50 * 1024 * 1024;

pub fn create_router_with_provider(
    api_keys: Arc<ApiKeyManager>,
    kiro_provider: Option<KiroProvider>,
    profile_arn: Option<String>,
) -> Router {
    let mut state = AppState::new(api_keys);
    if let Some(provider) = kiro_provider {
        state = state.with_kiro_provider(provider);
    }
    if let Some(arn) = profile_arn {
        state = state.with_profile_arn(arn);
    }

    let v1_routes = Router::new()
        .route("/models", get(get_models))
        .route("/messages", post(post_messages))
        .route("/messages/count_tokens", post(count_tokens))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let cc_v1_routes = Router::new()
        .route("/messages", post(post_messages_cc))
        .route("/messages/count_tokens", post(count_tokens))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .nest("/v1", v1_routes)
        .nest("/cc/v1", cc_v1_routes)
        .layer(cors_layer())
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .with_state(state)
}

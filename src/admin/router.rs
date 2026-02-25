use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};

use super::{
    handlers::{
        add_credential, create_api_key, delete_api_key, delete_credential, export_credential,
        export_credentials, get_all_credentials, get_api_stats, get_credential_balance,
        get_load_balancing_mode, get_request_logs, get_total_balance, list_api_keys, login,
        reset_failure_count, set_api_key_disabled, set_credential_disabled,
        set_credential_priority, set_load_balancing_mode,
    },
    middleware::{AdminState, admin_auth_middleware},
};

pub fn create_admin_router(state: AdminState) -> Router {
    let protected = Router::new()
        .route(
            "/credentials",
            get(get_all_credentials).post(add_credential),
        )
        .route("/credentials/export", get(export_credentials))
        .route("/credentials/{id}", delete(delete_credential))
        .route("/credentials/{id}/export", get(export_credential))
        .route("/credentials/{id}/disabled", post(set_credential_disabled))
        .route("/credentials/{id}/priority", post(set_credential_priority))
        .route("/credentials/{id}/reset", post(reset_failure_count))
        .route("/credentials/{id}/balance", get(get_credential_balance))
        .route("/balance/total", get(get_total_balance))
        .route(
            "/config/load-balancing",
            get(get_load_balancing_mode).put(set_load_balancing_mode),
        )
        .route("/apikeys", get(list_api_keys).post(create_api_key))
        .route("/apikeys/{id}", delete(delete_api_key))
        .route("/apikeys/{id}/disabled", post(set_api_key_disabled))
        .route("/stats", get(get_api_stats))
        .route("/logs", get(get_request_logs))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_auth_middleware,
        ));

    Router::new()
        .route("/auth/login", post(login))
        .merge(protected)
        .with_state(state)
}

//! Admin middleware and state

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
};
use chrono::{Duration, Utc};
use parking_lot::Mutex;
use uuid::Uuid;

use super::service::AdminService;
use super::types::AdminErrorResponse;
use crate::common::auth;

const SESSION_TTL_HOURS: i64 = 24;

#[derive(Debug, Clone)]
pub struct AdminSession {
    pub token: String,
    pub username: String,
    pub expires_at: String,
}

#[derive(Default)]
pub struct SessionManager {
    sessions: Mutex<HashMap<String, AdminSession>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&self, username: &str) -> AdminSession {
        let token = format!("adm_{}", Uuid::new_v4().simple());
        let expires_at = (Utc::now() + Duration::hours(SESSION_TTL_HOURS)).to_rfc3339();
        let session = AdminSession {
            token: token.clone(),
            username: username.to_string(),
            expires_at,
        };
        self.sessions.lock().insert(token, session.clone());
        session
    }

    pub fn validate(&self, token: &str) -> bool {
        self.cleanup_expired();
        self.sessions
            .lock()
            .get(token)
            .is_some_and(|s| s.expires_at > Utc::now().to_rfc3339())
    }

    pub fn cleanup_expired(&self) {
        let now = Utc::now().to_rfc3339();
        self.sessions.lock().retain(|_, s| s.expires_at > now);
    }
}

#[derive(Clone)]
pub struct AdminState {
    pub admin_username: String,
    pub admin_password: String,
    pub sessions: Arc<SessionManager>,
    pub service: Arc<AdminService>,
}

impl AdminState {
    pub fn new(
        admin_username: impl Into<String>,
        admin_password: impl Into<String>,
        service: AdminService,
    ) -> Self {
        Self {
            admin_username: admin_username.into(),
            admin_password: admin_password.into(),
            sessions: Arc::new(SessionManager::new()),
            service: Arc::new(service),
        }
    }

    pub fn verify_login(&self, username: &str, password: &str) -> bool {
        auth::constant_time_eq(username, &self.admin_username)
            && auth::constant_time_eq(password, &self.admin_password)
    }
}

pub async fn admin_auth_middleware(
    State(state): State<AdminState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let token = auth::extract_api_key(&request);

    match token {
        Some(t) if state.sessions.validate(&t) => next.run(request).await,
        _ => {
            let error = AdminErrorResponse::authentication_error();
            (StatusCode::UNAUTHORIZED, Json(error)).into_response()
        }
    }
}

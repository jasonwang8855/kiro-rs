use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::admin::AdminState;
use crate::admin::types::AddCredentialRequest;
use crate::http_client::{ProxyConfig, build_client};
use crate::model::config::Config;

const DEFAULT_IDC_REGION: &str = "us-east-1";
const BUILDER_ID_START_URL: &str = "https://view.awsapps.com/start";

#[derive(Clone)]
pub struct KiroOAuthWebState {
    admin: AdminState,
    config: Config,
    sessions: Arc<Mutex<HashMap<String, WebAuthSession>>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum SessionStatus {
    Pending,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
struct WebAuthSession {
    state_id: String,
    device_code: String,
    user_code: String,
    verification_uri_complete: String,
    expires_in: i64,
    started_at: chrono::DateTime<Utc>,
    status: SessionStatus,
    error: Option<String>,
    auth_method: String,
    client_id: String,
    client_secret: String,
    region: String,
    credential_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct StartQuery {
    method: String,
    #[serde(rename = "startUrl")]
    start_url: Option<String>,
    region: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StatusQuery {
    state: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTokenRequest {
    refresh_token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportTokenResponse {
    success: bool,
    message: String,
    credential_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterClientResponse {
    client_id: String,
    client_secret: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartDeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri_complete: String,
    expires_in: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTokenResponse {
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
struct OidcErrorResponse {
    error: Option<String>,
    error_description: Option<String>,
}

pub fn create_kiro_oauth_router(admin: AdminState, config: Config) -> Router {
    let state = KiroOAuthWebState {
        admin,
        config,
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    Router::new()
        .route("/", get(select_page))
        .route("/start", get(start_auth))
        .route("/start-json", post(start_auth_json))
        .route("/status", get(check_status))
        .route("/import", post(import_token))
        .with_state(state)
}

async fn select_page() -> impl IntoResponse {
    Html(SELECT_HTML)
}

async fn start_auth(
    State(state): State<KiroOAuthWebState>,
    Query(query): Query<StartQuery>,
) -> impl IntoResponse {
    let method = query.method.trim().to_lowercase();
    let (auth_method, start_url, region) = match method.as_str() {
        "builder-id" => (
            "builder-id".to_string(),
            BUILDER_ID_START_URL.to_string(),
            DEFAULT_IDC_REGION.to_string(),
        ),
        "idc" => {
            let start = query.start_url.unwrap_or_default().trim().to_string();
            if start.is_empty() {
                return error_html(StatusCode::BAD_REQUEST, "startUrl is required for IDC");
            }
            let region = query
                .region
                .unwrap_or_else(|| DEFAULT_IDC_REGION.to_string())
                .trim()
                .to_string();
            ("idc".to_string(), start, region)
        }
        _ => return error_html(StatusCode::BAD_REQUEST, "Unknown method"),
    };

    let client = match build_http_client(&state.config) {
        Ok(c) => c,
        Err(e) => return error_html(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };

    let register = match register_client(&client, &region).await {
        Ok(v) => v,
        Err(e) => return error_html(StatusCode::BAD_GATEWAY, &e),
    };

    let start = match start_device_authorization(
        &client,
        &region,
        &register.client_id,
        &register.client_secret,
        &start_url,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => return error_html(StatusCode::BAD_GATEWAY, &e),
    };

    let state_id = Uuid::new_v4().to_string();
    let session = WebAuthSession {
        state_id: state_id.clone(),
        device_code: start.device_code,
        user_code: start.user_code,
        verification_uri_complete: start.verification_uri_complete.clone(),
        expires_in: start.expires_in.max(60),
        started_at: Utc::now(),
        status: SessionStatus::Pending,
        error: None,
        auth_method,
        client_id: register.client_id,
        client_secret: register.client_secret,
        region,
        credential_id: None,
    };

    state.sessions.lock().insert(state_id, session.clone());

    Html(render_start_html(&session)).into_response()
}

async fn start_auth_json(
    State(state): State<KiroOAuthWebState>,
    Json(query): Json<StartQuery>,
) -> impl IntoResponse {
    let method = query.method.trim().to_lowercase();
    let (auth_method, start_url, region) = match method.as_str() {
        "builder-id" => (
            "builder-id".to_string(),
            BUILDER_ID_START_URL.to_string(),
            DEFAULT_IDC_REGION.to_string(),
        ),
        "idc" => {
            let start = query.start_url.unwrap_or_default().trim().to_string();
            if start.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "startUrl is required for IDC"})),
                )
                    .into_response();
            }
            let region = query
                .region
                .unwrap_or_else(|| DEFAULT_IDC_REGION.to_string())
                .trim()
                .to_string();
            ("idc".to_string(), start, region)
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Unknown method"})),
            )
                .into_response()
        }
    };

    let client = match build_http_client(&state.config) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    let register = match register_client(&client, &region).await {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": e}))).into_response()
        }
    };

    let start = match start_device_authorization(
        &client,
        &region,
        &register.client_id,
        &register.client_secret,
        &start_url,
    )
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": e}))).into_response()
        }
    };

    let state_id = Uuid::new_v4().to_string();
    let session = WebAuthSession {
        state_id: state_id.clone(),
        device_code: start.device_code,
        user_code: start.user_code.clone(),
        verification_uri_complete: start.verification_uri_complete.clone(),
        expires_in: start.expires_in.max(60),
        started_at: Utc::now(),
        status: SessionStatus::Pending,
        error: None,
        auth_method,
        client_id: register.client_id,
        client_secret: register.client_secret,
        region,
        credential_id: None,
    };

    state.sessions.lock().insert(state_id.clone(), session);

    Json(json!({
        "stateId": state_id,
        "userCode": start.user_code,
        "verificationUri": start.verification_uri_complete,
        "expiresIn": start.expires_in.max(60)
    }))
    .into_response()
}

async fn check_status(
    State(state): State<KiroOAuthWebState>,
    Query(query): Query<StatusQuery>,
) -> impl IntoResponse {
    let current = {
        let sessions = state.sessions.lock();
        match sessions.get(&query.state) {
            Some(s) => s.clone(),
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"status":"failed","error":"session not found"})),
                )
                    .into_response();
            }
        }
    };

    let mut next = current.clone();
    if matches!(next.status, SessionStatus::Pending) {
        let elapsed = (Utc::now() - next.started_at).num_seconds();
        if elapsed >= next.expires_in {
            next.status = SessionStatus::Failed;
            next.error = Some("authentication timed out".to_string());
        } else {
            let client = match build_http_client(&state.config) {
                Ok(c) => c,
                Err(e) => {
                    next.status = SessionStatus::Failed;
                    next.error = Some(e.to_string());
                    let mut sessions = state.sessions.lock();
                    sessions.insert(query.state.clone(), next.clone());
                    return Json(
                        json!({"status":"failed","error":next.error.clone().unwrap_or_default()}),
                    )
                    .into_response();
                }
            };

            match poll_device_token(
                &client,
                &next.region,
                &next.client_id,
                &next.client_secret,
                &next.device_code,
            )
            .await
            {
                PollResult::Pending => {}
                PollResult::SlowDown => {}
                PollResult::Token(token) => {
                    let req = AddCredentialRequest {
                        refresh_token: token.refresh_token,
                        auth_method: next.auth_method.clone(),
                        client_id: Some(next.client_id.clone()),
                        client_secret: Some(next.client_secret.clone()),
                        priority: 0,
                        region: Some(next.region.clone()),
                        auth_region: Some(next.region.clone()),
                        api_region: None,
                        machine_id: None,
                        email: None,
                        proxy_url: None,
                        proxy_username: None,
                        proxy_password: None,
                    };

                    match state.admin.service.add_credential(req).await {
                        Ok(result) => {
                            next.status = SessionStatus::Success;
                            next.credential_id = Some(result.credential_id);
                        }
                        Err(e) => {
                            next.status = SessionStatus::Failed;
                            next.error = Some(e.to_string());
                        }
                    }
                }
                PollResult::Failed(err) => {
                    next.status = SessionStatus::Failed;
                    next.error = Some(err);
                }
            }
        }
    }

    {
        let mut sessions = state.sessions.lock();
        sessions.insert(query.state.clone(), next.clone());
    }

    let remaining = (next.expires_in - (Utc::now() - next.started_at).num_seconds()).max(0);
    match next.status {
        SessionStatus::Pending => Json(json!({
            "status":"pending",
            "remaining_seconds": remaining
        }))
        .into_response(),
        SessionStatus::Success => Json(json!({
            "status":"success",
            "credential_id": next.credential_id
        }))
        .into_response(),
        SessionStatus::Failed => Json(json!({
            "status":"failed",
            "error": next.error.unwrap_or_else(|| "unknown error".to_string())
        }))
        .into_response(),
    }
}

async fn import_token(
    State(state): State<KiroOAuthWebState>,
    Json(payload): Json<ImportTokenRequest>,
) -> impl IntoResponse {
    let refresh_token = payload.refresh_token.trim();
    if refresh_token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ImportTokenResponse {
                success: false,
                message: "refreshToken is required".to_string(),
                credential_id: None,
            }),
        )
            .into_response();
    }

    let req = AddCredentialRequest {
        refresh_token: refresh_token.to_string(),
        auth_method: "social".to_string(),
        client_id: None,
        client_secret: None,
        priority: 0,
        region: None,
        auth_region: None,
        api_region: None,
        machine_id: None,
        email: None,
        proxy_url: None,
        proxy_username: None,
        proxy_password: None,
    };

    match state.admin.service.add_credential(req).await {
        Ok(result) => (
            StatusCode::OK,
            Json(ImportTokenResponse {
                success: true,
                message: "Token imported and refreshed successfully".to_string(),
                credential_id: Some(result.credential_id),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ImportTokenResponse {
                success: false,
                message: e.to_string(),
                credential_id: None,
            }),
        )
            .into_response(),
    }
}

fn error_html(status: StatusCode, message: &str) -> axum::response::Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!(
            "<html><body><h2>Kiro OAuth Error</h2><p>{}</p><a href=\"/v0/oauth/kiro\">Back</a></body></html>",
            message
        ),
    )
        .into_response()
}

fn render_start_html(session: &WebAuthSession) -> String {
    format!(
        r##"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width,initial-scale=1" />
  <title>Kiro OAuth 验证中</title>
  <style>
    :root {{ color-scheme: light; }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      font-family: "Segoe UI", "PingFang SC", sans-serif;
      background:
        radial-gradient(circle at 15% 15%, rgba(14, 165, 233, 0.24), transparent 30%),
        radial-gradient(circle at 85% 10%, rgba(16, 185, 129, 0.18), transparent 32%),
        linear-gradient(140deg, #f8fafc, #eff6ff);
      display: grid;
      place-items: center;
      padding: 24px 12px;
    }}
    .panel {{
      width: 100%;
      max-width: 760px;
      border: 1px solid #dbeafe;
      border-radius: 20px;
      background: rgba(255, 255, 255, 0.95);
      box-shadow: 0 20px 70px rgba(2, 132, 199, 0.16);
      padding: 24px;
    }}
    .title {{ margin: 0 0 8px; font-size: 28px; color: #0f172a; }}
    .desc {{ margin: 0 0 20px; color: #475569; font-size: 14px; }}
    .grid {{ display: grid; gap: 14px; }}
    .card {{
      border: 1px solid #e2e8f0;
      border-radius: 14px;
      padding: 16px;
      background: #fff;
    }}
    .step {{ color: #334155; font-size: 13px; margin-bottom: 8px; font-weight: 600; }}
    .code {{
      font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      font-size: 28px;
      font-weight: 700;
      color: #0369a1;
      letter-spacing: 3px;
      background: #f0f9ff;
      border: 1px dashed #38bdf8;
      border-radius: 10px;
      padding: 10px 12px;
      text-align: center;
    }}
    .btn {{
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 100%;
      padding: 12px 14px;
      border-radius: 10px;
      border: 0;
      font-weight: 600;
      background: linear-gradient(135deg, #0284c7, #2563eb);
      color: #fff;
      text-decoration: none;
      transition: transform .15s ease, box-shadow .15s ease;
      box-shadow: 0 8px 20px rgba(37, 99, 235, 0.24);
    }}
    .btn:hover {{ transform: translateY(-1px); }}
    .status {{
      margin-top: 8px;
      padding: 12px;
      border-radius: 10px;
      font-size: 14px;
      background: #f8fafc;
      color: #0f172a;
      border: 1px solid #e2e8f0;
    }}
    .ok {{ background: #dcfce7; border-color: #86efac; color: #166534; }}
    .bad {{ background: #fee2e2; border-color: #fca5a5; color: #991b1b; }}
  </style>
</head>
<body>
  <div class="panel">
    <h1 class="title">Kiro OAuth 验证</h1>
    <p class="desc">请在 AWS 页面完成登录。验证成功后将自动导入凭证并返回后台。</p>
    <div class="grid">
      <div class="card">
        <div class="step">步骤 1：打开 AWS 验证页面</div>
        <a class="btn" href="{auth_url}" target="_blank">打开授权页面</a>
      </div>
      <div class="card">
        <div class="step">步骤 2：输入验证码</div>
        <div class="code">{user_code}</div>
      </div>
      <div class="card">
        <div id="status" class="status">等待授权完成...</div>
        <div id="timer" class="status">剩余时间：{expires}s</div>
      </div>
    </div>
  </div>
  <script>
    const state = "{state_id}";
    const statusBox = document.getElementById("status");
    const timerBox = document.getElementById("timer");
    async function poll() {{
      const res = await fetch("/v0/oauth/kiro/status?state=" + encodeURIComponent(state));
      const data = await res.json();
      if (data.status === "success") {{
        statusBox.className = "status ok";
        statusBox.innerText = "验证成功，已自动导入凭证。Credential ID: " + data.credential_id;
        timerBox.className = "status ok";
        timerBox.innerText = "你现在可以关闭此页面。";
        return;
      }}
      if (data.status === "failed") {{
        statusBox.className = "status bad";
        statusBox.innerText = "验证失败: " + (data.error || "unknown error");
        timerBox.className = "status bad";
        timerBox.innerText = "请返回后台重试。";
        return;
      }}
      timerBox.innerText = "剩余时间：" + data.remaining_seconds + "s";
      setTimeout(poll, 3000);
    }}
    setTimeout(() => document.querySelector(".btn").click(), 300);
    poll();
  </script>
</body>
</html>"##,
        auth_url = session.verification_uri_complete,
        user_code = session.user_code,
        expires = session.expires_in,
        state_id = session.state_id
    )
}

fn build_http_client(config: &Config) -> anyhow::Result<reqwest::Client> {
    let proxy = config.proxy_url.as_ref().map(|url| {
        let mut p = ProxyConfig::new(url);
        if let (Some(username), Some(password)) = (&config.proxy_username, &config.proxy_password) {
            p = p.with_auth(username, password);
        }
        p
    });
    build_client(proxy.as_ref(), 60, config.tls_backend)
}

async fn register_client(
    client: &reqwest::Client,
    region: &str,
) -> Result<RegisterClientResponse, String> {
    let endpoint = format!("https://oidc.{}.amazonaws.com/client/register", region);
    let body = json!({
        "clientName": "Kiro IDE",
        "clientType": "public",
        "scopes": [
            "codewhisperer:completions",
            "codewhisperer:analysis",
            "codewhisperer:conversations",
            "codewhisperer:transformations",
            "codewhisperer:taskassist"
        ],
        "grantTypes": ["urn:ietf:params:oauth:grant-type:device_code", "refresh_token"]
    });

    let resp = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("User-Agent", "KiroIDE")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("register client failed: {} {}", status, text));
    }
    resp.json::<RegisterClientResponse>()
        .await
        .map_err(|e| e.to_string())
}

async fn start_device_authorization(
    client: &reqwest::Client,
    region: &str,
    client_id: &str,
    client_secret: &str,
    start_url: &str,
) -> Result<StartDeviceAuthResponse, String> {
    let endpoint = format!("https://oidc.{}.amazonaws.com/device_authorization", region);
    let body = json!({
        "clientId": client_id,
        "clientSecret": client_secret,
        "startUrl": start_url
    });
    let resp = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("User-Agent", "KiroIDE")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!(
            "start device authorization failed: {} {}",
            status, text
        ));
    }
    resp.json::<StartDeviceAuthResponse>()
        .await
        .map_err(|e| e.to_string())
}

enum PollResult {
    Pending,
    SlowDown,
    Token(CreateTokenResponse),
    Failed(String),
}

async fn poll_device_token(
    client: &reqwest::Client,
    region: &str,
    client_id: &str,
    client_secret: &str,
    device_code: &str,
) -> PollResult {
    let endpoint = format!("https://oidc.{}.amazonaws.com/token", region);
    let body = json!({
        "clientId": client_id,
        "clientSecret": client_secret,
        "deviceCode": device_code,
        "grantType": "urn:ietf:params:oauth:grant-type:device_code"
    });
    let resp = match client
        .post(endpoint)
        .header("Content-Type", "application/json")
        .header("User-Agent", "KiroIDE")
        .json(&body)
        .send()
        .await
    {
        Ok(v) => v,
        Err(e) => return PollResult::Failed(e.to_string()),
    };

    let status = resp.status();
    if status.is_success() {
        return match resp.json::<CreateTokenResponse>().await {
            Ok(v) => PollResult::Token(v),
            Err(e) => PollResult::Failed(e.to_string()),
        };
    }

    let text = resp.text().await.unwrap_or_default();
    if let Ok(err_obj) = serde_json::from_str::<OidcErrorResponse>(&text) {
        if err_obj.error.as_deref() == Some("authorization_pending") {
            return PollResult::Pending;
        }
        if err_obj.error.as_deref() == Some("slow_down") {
            return PollResult::SlowDown;
        }
        let msg = err_obj.error_description.unwrap_or_else(|| {
            err_obj
                .error
                .unwrap_or_else(|| format!("token error: {}", status))
        });
        return PollResult::Failed(msg);
    }

    PollResult::Failed(format!("token request failed: {} {}", status, text))
}

const SELECT_HTML: &str = r##"<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width,initial-scale=1" />
  <title>Kiro OAuth Login</title>
  <style>
    :root { color-scheme: light; }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      font-family: "Segoe UI", "PingFang SC", sans-serif;
      background:
        radial-gradient(circle at 8% 10%, rgba(59, 130, 246, 0.24), transparent 28%),
        radial-gradient(circle at 95% 0%, rgba(16, 185, 129, 0.2), transparent 32%),
        linear-gradient(140deg, #eef2ff, #f8fafc);
      padding: 28px 14px;
      display: grid;
      place-items: start center;
    }
    .wrap {
      width: 100%;
      max-width: 900px;
      border: 1px solid #dbeafe;
      border-radius: 22px;
      background: rgba(255,255,255,.96);
      box-shadow: 0 24px 80px rgba(30, 64, 175, .14);
      padding: 24px;
    }
    h1 { margin: 0 0 6px; font-size: 30px; color: #0f172a; }
    .lead { margin: 0 0 18px; color: #475569; font-size: 14px; }
    .grid { display: grid; gap: 12px; }
    .box {
      border: 1px solid #e2e8f0;
      border-radius: 14px;
      padding: 16px;
      background: #fff;
    }
    .box h3 { margin: 0 0 10px; color: #0f172a; }
    .btn {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 100%;
      padding: 11px 14px;
      background: linear-gradient(135deg, #0284c7, #2563eb);
      color: #fff;
      border: 0;
      border-radius: 10px;
      cursor: pointer;
      text-decoration: none;
      font-weight: 600;
    }
    .btn-alt {
      background: linear-gradient(135deg, #0f766e, #0ea5e9);
    }
    input, textarea {
      width: 100%;
      padding: 9px 10px;
      margin-top: 5px;
      margin-bottom: 10px;
      border: 1px solid #cbd5e1;
      border-radius: 8px;
      font-size: 14px;
      background: #fff;
    }
    .status {
      padding: 10px;
      border-radius: 8px;
      margin-top: 8px;
      font-size: 13px;
      border: 1px solid #e2e8f0;
    }
    .tip {
      margin-top: 12px;
      padding: 12px;
      border-radius: 10px;
      background: #f0f9ff;
      border: 1px solid #bae6fd;
      color: #0c4a6e;
      font-size: 13px;
      line-height: 1.5;
    }
  </style>
</head>
<body>
  <div class="wrap">
    <h1>Kiro Authentication</h1>
    <p class="lead">支持 AWS Builder ID / IDC 登录，导入后会强制刷新校验并自动写入凭证。</p>
    <div class="grid">
      <div class="box">
        <h3>AWS Builder ID（推荐）</h3>
        <a class="btn" href="/v0/oauth/kiro/start?method=builder-id">开始 Builder ID 登录</a>
      </div>
      <div class="box">
        <h3>AWS Identity Center（IDC）</h3>
        <form action="/v0/oauth/kiro/start" method="get">
          <input type="hidden" name="method" value="idc"/>
          <label>Start URL</label>
          <input name="startUrl" placeholder="https://your-org.awsapps.com/start" required />
          <label>Region</label>
          <input name="region" value="us-east-1" />
          <button class="btn btn-alt" type="submit">开始 IDC 登录</button>
        </form>
      </div>
      <div class="box">
        <h3>从 Kiro IDE 导入 RefreshToken</h3>
        <form id="importForm">
          <label>refreshToken</label>
          <textarea id="refreshToken" rows="4" placeholder="粘贴 refreshToken"></textarea>
          <button class="btn" type="submit">导入并强制刷新验证</button>
        </form>
        <div id="result"></div>
      </div>
    </div>
    <div class="tip">
      提示：如果从后台进入本页面，完成验证后会自动返回并刷新凭证列表。<br/>
      Token 文件位置示例：<code>~/.kiro/kiro-auth-token.json</code>
    </div>
  </div>
  <script>
    document.getElementById("importForm").addEventListener("submit", async (e) => {
      e.preventDefault();
      const token = document.getElementById("refreshToken").value.trim();
      const out = document.getElementById("result");
      out.innerHTML = "";
      const resp = await fetch("/v0/oauth/kiro/import", {
        method: "POST",
        headers: {"Content-Type":"application/json"},
        body: JSON.stringify({refreshToken: token})
      });
      const data = await resp.json();
      out.className = "status";
      out.textContent = data.message + (data.credentialId ? (" (credentialId=" + data.credentialId + ")") : "");
      out.style.background = resp.ok ? "#dcfce7" : "#fee2e2";
      out.style.borderColor = resp.ok ? "#86efac" : "#fca5a5";
    });
  </script>
</body>
</html>"##;

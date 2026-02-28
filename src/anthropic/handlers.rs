//! Anthropic API Handler 函数

use std::convert::Infallible;
use std::time::Instant;

use crate::apikeys::AuthenticatedApiKey;
use crate::kiro::model::events::Event;
use crate::kiro::model::requests::kiro::KiroRequest;
use crate::kiro::parser::decoder::EventStreamDecoder;
use crate::kiro::provider::StickyError;
use crate::request_log::{RequestLog, RequestLogEntry};
use crate::sticky::{RequestIdentity, StreamGuard};
use crate::token;
use anyhow::Error;
use axum::{
    Json as JsonExtractor,
    body::Body,
    extract::{Extension, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use bytes::Bytes;
use futures::{Stream, StreamExt, stream};
use serde_json::json;
use std::time::Duration;
use tokio::time::interval;
use uuid::Uuid;

use super::converter::{ConversionError, convert_request};
use super::middleware::AppState;
use super::stream::{BufferedStreamContext, SseEvent, StreamContext};
use super::types::{
    CountTokensRequest, CountTokensResponse, ErrorResponse, MessagesRequest, Model, ModelsResponse,
    OutputConfig, Thinking,
};
use super::websearch;

/// 路由决策结果
#[derive(Debug)]
enum RouteDecision {
    /// 绑定指定凭据（fixed 模式）
    Fixed(u64),
    /// Sticky 模式，用 X-User-Id 做身份绑定
    Sticky(RequestIdentity),
    /// 走全局负载均衡（priority 或 balanced）
    Global,
}

/// 解析路由决策
///
/// 决策逻辑：
/// 1. Fixed + credential_id → RouteDecision::Fixed(id)
/// 2. Auto + sticky 模式 + 有 X-User-Id → RouteDecision::Sticky(identity)
/// 3. 其余 → RouteDecision::Global
fn resolve_route(
    auth: &AuthenticatedApiKey,
    headers: &HeaderMap,
    state: &AppState,
    provider: &crate::kiro::provider::KiroProvider,
    metadata: Option<&crate::anthropic::types::Metadata>,
) -> RouteDecision {
    use crate::apikeys::RoutingMode;

    // Fixed 模式：直接绑定凭据
    if auth.routing_mode == RoutingMode::Fixed {
        if let Some(cred_id) = auth.credential_id {
            return RouteDecision::Fixed(cred_id);
        }
        // Fixed 但没有 credential_id（不应该发生，admin 校验过了），退化为 Global
    }

    // 检查是否 sticky 模式
    let is_sticky =
        state.sticky_tracker.is_some() && provider.token_manager().get_load_balancing_mode() == "sticky";

    if is_sticky {
        // 提取 X-User-Id header
        if let Some(user_id) = headers.get("x-user-id").and_then(|v| v.to_str().ok()) {
            if !user_id.is_empty() {
                // 用 api_key_id + user_id 组合作为 sticky 身份，防止跨 key 伪造
                let sticky_identity = format!("{}:{}", auth.key_id, user_id);
                return RouteDecision::Sticky(RequestIdentity {
                    api_key: sticky_identity,
                    session_id: metadata.and_then(|m| m.user_id.clone()),
                });
            }
        }
        // 没有 X-User-Id，走 Global（当前行为是 priority）
    }

    RouteDecision::Global
}

/// 将 KiroProvider 错误映射为 HTTP 响应
fn map_provider_error(err: Error) -> Response {
    let err_str = err.to_string();

    // 上下文窗口满了（对话历史累积超出模型上下文窗口限制）
    if err_str.contains("CONTENT_LENGTH_EXCEEDS_THRESHOLD") {
        tracing::warn!(error = %err, "上游拒绝请求：上下文窗口已满（不应重试）");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_request_error",
                "Context window is full. Reduce conversation history, system prompt, or tools.",
            )),
        )
            .into_response();
    }

    // 单次输入太长（请求体本身超出上游限制）
    if err_str.contains("Input is too long") {
        tracing::warn!(error = %err, "上游拒绝请求：输入过长（不应重试）");
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_request_error",
                "Input is too long. Reduce the size of your messages.",
            )),
        )
            .into_response();
    }
    tracing::error!("Kiro API 调用失败: {}", err);
    (
        StatusCode::BAD_GATEWAY,
        Json(ErrorResponse::new(
            "api_error",
            format!("上游 API 调用失败: {}", err),
        )),
    )
        .into_response()
}

/// 将 StickyError 映射为 HTTP 响应
fn map_sticky_error(err: StickyError) -> Response {
    match err {
        StickyError::AllFull { retry_after_secs } => {
            let retry_after = format!("{}", retry_after_secs.ceil() as u64);
            Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header("Retry-After", retry_after)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_string(&ErrorResponse::new(
                        "overloaded_error",
                        "All credentials at capacity. Please retry later.",
                    ))
                    .unwrap(),
                ))
                .unwrap()
        }
        StickyError::NoTracker => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("internal_error", "Sticky tracker not configured")),
        )
            .into_response(),
        StickyError::ApiError(e) => map_provider_error(e),
    }
}

/// GET /v1/models
///
/// 返回可用的模型列表
pub async fn get_models() -> impl IntoResponse {
    tracing::info!("Received GET /v1/models request");

    let models = vec![
        Model {
            id: "claude-sonnet-4-5-20250929".to_string(),
            object: "model".to_string(),
            created: 1727568000,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Sonnet 4.5".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-sonnet-4-5-20250929-thinking".to_string(),
            object: "model".to_string(),
            created: 1727568000,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Sonnet 4.5 (Thinking)".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-opus-4-5-20251101".to_string(),
            object: "model".to_string(),
            created: 1730419200,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Opus 4.5".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-opus-4-5-20251101-thinking".to_string(),
            object: "model".to_string(),
            created: 1730419200,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Opus 4.5 (Thinking)".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-sonnet-4-6".to_string(),
            object: "model".to_string(),
            created: 1770314400,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Sonnet 4.6".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-sonnet-4-6-thinking".to_string(),
            object: "model".to_string(),
            created: 1770314400,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Sonnet 4.6 (Thinking)".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-opus-4-6".to_string(),
            object: "model".to_string(),
            created: 1770314400,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Opus 4.6".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-opus-4-6-thinking".to_string(),
            object: "model".to_string(),
            created: 1770314400,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Opus 4.6 (Thinking)".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-haiku-4-5-20251001".to_string(),
            object: "model".to_string(),
            created: 1727740800,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Haiku 4.5".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
        Model {
            id: "claude-haiku-4-5-20251001-thinking".to_string(),
            object: "model".to_string(),
            created: 1727740800,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Haiku 4.5 (Thinking)".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 32000,
        },
    ];

    Json(ModelsResponse {
        object: "list".to_string(),
        data: models,
    })
}

/// POST /v1/messages
///
/// 创建消息（对话）
pub async fn post_messages(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedApiKey>,
    headers: HeaderMap,
    JsonExtractor(mut payload): JsonExtractor<MessagesRequest>,
) -> Response {
    tracing::info!(
        model = %payload.model,
        max_tokens = %payload.max_tokens,
        stream = %payload.stream,
        message_count = %payload.messages.len(),
        "Received POST /v1/messages request"
    );
    // 检查 KiroProvider 是否可用
    let provider = match &state.kiro_provider {
        Some(p) => p.clone(),
        None => {
            tracing::error!("KiroProvider 未配置");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new(
                    "service_unavailable",
                    "Kiro API provider not configured",
                )),
            )
                .into_response();
        }
    };

    // 检测模型名是否包含 "thinking" 后缀，若包含则覆写 thinking 配置
    override_thinking_from_model_name(&mut payload);

    // 检查是否为 WebSearch 请求
    if websearch::has_web_search_tool(&payload) {
        tracing::info!("检测到 WebSearch 工具，路由到 WebSearch 处理");

        // 估算输入 tokens
        let input_tokens = token::count_all_tokens(
            payload.model.clone(),
            payload.system.clone(),
            payload.messages.clone(),
            payload.tools.clone(),
        ) as i32;

        // WebSearch 路径也执行路由决策，避免绕过 fixed/sticky
        let route = resolve_route(
            &auth,
            &headers,
            &state,
            provider.as_ref(),
            payload.metadata.as_ref(),
        );
        let fixed_credential_id = match route {
            RouteDecision::Fixed(credential_id) => Some(credential_id),
            RouteDecision::Sticky(identity) => state
                .sticky_tracker
                .as_ref()
                .and_then(|tracker| tracker.get_bound_credential(&identity.api_key)),
            RouteDecision::Global => None,
        };

        return websearch::handle_websearch_request(
            provider,
            &payload,
            input_tokens,
            fixed_credential_id,
        )
        .await;
    }

    // 转换请求
    let conversion_result = match convert_request(&payload) {
        Ok(result) => result,
        Err(e) => {
            let (error_type, message) = match &e {
                ConversionError::UnsupportedModel(model) => {
                    ("invalid_request_error", format!("模型不支持: {}", model))
                }
                ConversionError::EmptyMessages => {
                    ("invalid_request_error", "消息列表为空".to_string())
                }
            };
            tracing::warn!("请求转换失败: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(error_type, message)),
            )
                .into_response();
        }
    };

    // 构建 Kiro 请求
    let kiro_request = KiroRequest {
        conversation_state: conversion_result.conversation_state,
        profile_arn: state.profile_arn.clone(),
    };

    let request_body = match serde_json::to_string(&kiro_request) {
        Ok(body) => body,
        Err(e) => {
            tracing::error!("序列化请求失败: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    format!("序列化请求失败: {}", e),
                )),
            )
                .into_response();
        }
    };

    tracing::debug!("Kiro request body: {}", request_body);

    let message_count = payload.messages.len();
    let start = Instant::now();
    let log_request_body = if state.request_log.as_ref().is_some_and(|l| l.is_enabled()) {
        serde_json::to_string(&payload).unwrap_or_default()
    } else {
        String::new()
    };

    // 估算输入 tokens
    let input_tokens = token::count_all_tokens(
        payload.model.clone(),
        payload.system,
        payload.messages,
        payload.tools,
    ) as i32;

    // 检查是否启用了thinking
    let thinking_enabled = payload
        .thinking
        .as_ref()
        .map(|t| t.is_enabled())
        .unwrap_or(false);

    let route = resolve_route(
        &auth,
        &headers,
        &state,
        provider.as_ref(),
        payload.metadata.as_ref(),
    );

    match route {
        RouteDecision::Fixed(credential_id) => {
            if payload.stream {
                let response = match provider
                    .call_api_stream_fixed(&request_body, credential_id)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => return map_provider_error(e),
                };
                let mut ctx =
                    StreamContext::new_with_thinking(&payload.model, input_tokens, thinking_enabled);
                let initial_events = ctx.generate_initial_events();
                let stream = create_sse_stream(
                    response,
                    ctx,
                    initial_events,
                    state.api_keys.clone(),
                    auth.key_id.clone(),
                    state.request_log.clone(),
                    payload.model.to_string(),
                    message_count,
                    start,
                    log_request_body,
                );
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .header(header::CONNECTION, "keep-alive")
                    .body(Body::from_stream(stream))
                    .unwrap()
            } else {
                handle_non_stream_request_fixed(
                    provider,
                    state.api_keys.clone(),
                    &auth.key_id,
                    &request_body,
                    &payload.model,
                    input_tokens,
                    state.request_log.clone(),
                    message_count,
                    start,
                    log_request_body,
                    credential_id,
                )
                .await
            }
        }
        RouteDecision::Sticky(identity) => {
            if payload.stream {
                // Sticky 模式：通过 StickyTracker 选择凭据
                let (response, guard) = match provider
                    .call_api_stream_sticky(&request_body, &identity)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => return map_sticky_error(e),
                };

                let mut ctx =
                    StreamContext::new_with_thinking(&payload.model, input_tokens, thinking_enabled);
                let initial_events = ctx.generate_initial_events();

                let stream = create_sse_stream_with_guard(
                    response,
                    ctx,
                    initial_events,
                    state.api_keys.clone(),
                    auth.key_id.clone(),
                    state.request_log.clone(),
                    payload.model.to_string(),
                    message_count,
                    start,
                    log_request_body,
                    guard,
                );

                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .header(header::CONNECTION, "keep-alive")
                    .body(Body::from_stream(stream))
                    .unwrap()
            } else {
                // sticky 非流式：优先使用已绑定凭据，否则回退到 Global
                let bound_cred = state
                    .sticky_tracker
                    .as_ref()
                    .and_then(|tracker| tracker.get_bound_credential(&identity.api_key));
                if let Some(credential_id) = bound_cred {
                    handle_non_stream_request_fixed(
                        provider,
                        state.api_keys.clone(),
                        &auth.key_id,
                        &request_body,
                        &payload.model,
                        input_tokens,
                        state.request_log.clone(),
                        message_count,
                        start,
                        log_request_body,
                        credential_id,
                    )
                    .await
                } else {
                    handle_non_stream_request(
                        provider,
                        state.api_keys.clone(),
                        &auth.key_id,
                        &request_body,
                        &payload.model,
                        input_tokens,
                        state.request_log.clone(),
                        message_count,
                        start,
                        log_request_body,
                    )
                    .await
                }
            }
        }
        RouteDecision::Global => {
            if payload.stream {
                handle_stream_request(
                    provider,
                    state.api_keys.clone(),
                    auth.key_id.clone(),
                    &request_body,
                    &payload.model,
                    input_tokens,
                    thinking_enabled,
                    state.request_log.clone(),
                    message_count,
                    start,
                    log_request_body,
                )
                .await
            } else {
                handle_non_stream_request(
                    provider,
                    state.api_keys.clone(),
                    &auth.key_id,
                    &request_body,
                    &payload.model,
                    input_tokens,
                    state.request_log.clone(),
                    message_count,
                    start,
                    log_request_body,
                )
                .await
            }
        }
    }
}

/// 处理流式请求
async fn handle_stream_request(
    provider: std::sync::Arc<crate::kiro::provider::KiroProvider>,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    key_id: String,
    request_body: &str,
    model: &str,
    input_tokens: i32,
    thinking_enabled: bool,
    request_log: Option<std::sync::Arc<RequestLog>>,
    message_count: usize,
    start: Instant,
    log_request_body: String,
) -> Response {
    // 调用 Kiro API（支持多凭据故障转移）
    let response = match provider.call_api_stream(request_body).await {
        Ok(resp) => resp,
        Err(e) => return map_provider_error(e),
    };

    // 创建流处理上下文
    let mut ctx = StreamContext::new_with_thinking(model, input_tokens, thinking_enabled);

    // 生成初始事件（内部状态初始化，纯文本模式不发送）
    let initial_events = ctx.generate_initial_events();

    // 创建 SSE 流
    let stream = create_sse_stream(response, ctx, initial_events, api_keys, key_id, request_log, model.to_string(), message_count, start, log_request_body);

    // 返回 SSE 流式响应
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// Ping 事件间隔（25秒）
const PING_INTERVAL_SECS: u64 = 25;

/// 将 SSE 事件列表转换为标准 SSE 字节流
fn events_to_sse_bytes(events: Vec<SseEvent>) -> Vec<Result<Bytes, Infallible>> {
    events
        .into_iter()
        .map(|e| Ok(Bytes::from(e.to_sse_string())))
        .collect()
}

/// 流式请求日志上下文
struct StreamLogCtx {
    request_log: Option<std::sync::Arc<RequestLog>>,
    model: String,
    message_count: usize,
    key_id: String,
    start: Instant,
    request_body: String,
    response_events: Vec<serde_json::Value>,
}

impl StreamLogCtx {
    fn record(&self, input: i32, output: i32, token_source: &str, status: &str) {
        if let Some(log) = &self.request_log {
            log.push(RequestLogEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                model: self.model.clone(),
                stream: true,
                message_count: self.message_count,
                input_tokens: input,
                output_tokens: output,
                token_source: token_source.to_string(),
                duration_ms: self.start.elapsed().as_millis() as u64,
                status: status.to_string(),
                api_key_id: self.key_id.clone(),
                request_body: self.request_body.clone(),
                response_body: serde_json::to_string(&self.response_events).unwrap_or_default(),
            });
        }
    }
}

/// 创建 SSE 事件流
fn create_sse_stream(
    response: reqwest::Response,
    ctx: StreamContext,
    initial_events: Vec<SseEvent>,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    key_id: String,
    request_log: Option<std::sync::Arc<RequestLog>>,
    model: String,
    message_count: usize,
    start: Instant,
    log_request_body: String,
) -> impl Stream<Item = Result<Bytes, Infallible>> {
    // 初始事件
    let initial_stream = stream::iter(events_to_sse_bytes(initial_events));

    let log_ctx = StreamLogCtx { request_log, model, message_count, key_id: key_id.clone(), start, request_body: log_request_body, response_events: Vec::new() };

    // 然后处理 Kiro 响应流，同时每25秒发送 ping 保活
    let body_stream = response.bytes_stream();

    let processing_stream = stream::unfold(
        (body_stream, ctx, EventStreamDecoder::new(), false, interval(Duration::from_secs(PING_INTERVAL_SECS)), api_keys, key_id, false, log_ctx),
        |(mut body_stream, mut ctx, mut decoder, finished, mut ping_interval, api_keys, key_id, usage_recorded, mut log_ctx)| async move {
            if finished {
                return None;
            }

            // 使用 select! 同时等待数据和 ping 定时器
            tokio::select! {
                // 处理数据流
                chunk_result = body_stream.next() => {
                    match chunk_result {
                        Some(Ok(chunk)) => {
                            // 解码事件
                            if let Err(e) = decoder.feed(&chunk) {
                                tracing::warn!("缓冲区溢出: {}", e);
                            }

                            let mut events = Vec::new();
                            for result in decoder.decode_iter() {
                                match result {
                                    Ok(frame) => {
                                        if let Ok(event) = Event::from_frame(frame) {
                                            let sse_events = ctx.process_kiro_event(&event);
                                            // 收集事件数据用于日志
                                            for se in &sse_events {
                                                log_ctx.response_events.push(json!({
                                                    "event": se.event,
                                                    "data": se.data,
                                                }));
                                            }
                                            events.extend(sse_events);
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!("解码事件失败: {}", e);
                                    }
                                }
                            }

                            // 转换为 SSE 字节流
                            let bytes = events_to_sse_bytes(events);

                            Some((stream::iter(bytes), (body_stream, ctx, decoder, false, ping_interval, api_keys, key_id, usage_recorded, log_ctx)))
                        }
                        Some(Err(e)) => {
                            tracing::error!("读取响应流失败: {}", e);
                            // 记录用量
                            if !usage_recorded {
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                log_ctx.record(input, output, ctx.token_source(), &format!("error: {}", e));
                            }
                            let final_events = ctx.generate_final_events();
                            let bytes = events_to_sse_bytes(final_events);
                            Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, true, log_ctx)))
                        }
                        None => {
                            // 流结束，记录用量
                            if !usage_recorded {
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                log_ctx.record(input, output, ctx.token_source(), "success");
                            }
                            let final_events = ctx.generate_final_events();
                            let bytes = events_to_sse_bytes(final_events);
                            Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, true, log_ctx)))
                        }
                    }
                }
                // 发送 ping 保活（纯文本模式发送空格，不影响内容）
                _ = ping_interval.tick() => {
                    tracing::trace!("发送 ping 保活事件");
                    let bytes: Vec<Result<Bytes, Infallible>> = vec![Ok(Bytes::from_static(b"event: ping\ndata: {\"type\": \"ping\"}\n\n"))];
                    Some((stream::iter(bytes), (body_stream, ctx, decoder, false, ping_interval, api_keys, key_id, usage_recorded, log_ctx)))
                }
            }
        },
    )
    .flatten();

    initial_stream.chain(processing_stream)
}

/// 创建带 StreamGuard 的 SSE 事件流（sticky 模式用）
fn create_sse_stream_with_guard(
    response: reqwest::Response,
    ctx: StreamContext,
    initial_events: Vec<SseEvent>,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    key_id: String,
    request_log: Option<std::sync::Arc<RequestLog>>,
    model: String,
    message_count: usize,
    start: Instant,
    log_request_body: String,
    mut guard: StreamGuard,
) -> impl Stream<Item = Result<Bytes, Infallible>> {
    // 激活流（上游已成功响应）
    guard.activate();

    let initial_stream = stream::iter(events_to_sse_bytes(initial_events));
    let log_ctx = StreamLogCtx { request_log, model, message_count, key_id: key_id.clone(), start, request_body: log_request_body, response_events: Vec::new() };
    let body_stream = response.bytes_stream();

    let processing_stream = stream::unfold(
        (body_stream, ctx, EventStreamDecoder::new(), false, interval(Duration::from_secs(PING_INTERVAL_SECS)), api_keys, key_id, false, log_ctx, guard),
        |(mut body_stream, mut ctx, mut decoder, finished, mut ping_interval, api_keys, key_id, usage_recorded, mut log_ctx, guard)| async move {
            if finished {
                return None;
            }

            tokio::select! {
                chunk_result = body_stream.next() => {
                    match chunk_result {
                        Some(Ok(chunk)) => {
                            guard.touch();
                            if let Err(e) = decoder.feed(&chunk) {
                                tracing::warn!("缓冲区溢出: {}", e);
                            }
                            let mut events = Vec::new();
                            for result in decoder.decode_iter() {
                                match result {
                                    Ok(frame) => {
                                        if let Ok(event) = Event::from_frame(frame) {
                                            let sse_events = ctx.process_kiro_event(&event);
                                            for se in &sse_events {
                                                log_ctx.response_events.push(json!({
                                                    "event": se.event,
                                                    "data": se.data,
                                                }));
                                            }
                                            events.extend(sse_events);
                                        }
                                    }
                                    Err(e) => { tracing::warn!("解码事件失败: {}", e); }
                                }
                            }
                            let bytes = events_to_sse_bytes(events);
                            Some((stream::iter(bytes), (body_stream, ctx, decoder, false, ping_interval, api_keys, key_id, usage_recorded, log_ctx, guard)))
                        }
                        Some(Err(e)) => {
                            tracing::error!("读取响应流失败: {}", e);
                            if !usage_recorded {
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                log_ctx.record(input, output, ctx.token_source(), &format!("error: {}", e));
                            }
                            let final_events = ctx.generate_final_events();
                            let bytes = events_to_sse_bytes(final_events);
                            // guard will be dropped here, deactivating the stream
                            Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, true, log_ctx, guard)))
                        }
                        None => {
                            if !usage_recorded {
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                log_ctx.record(input, output, ctx.token_source(), "success");
                            }
                            let final_events = ctx.generate_final_events();
                            let bytes = events_to_sse_bytes(final_events);
                            Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, true, log_ctx, guard)))
                        }
                    }
                }
                _ = ping_interval.tick() => {
                    tracing::trace!("发送 ping 保活事件");
                    let bytes: Vec<Result<Bytes, Infallible>> = vec![Ok(Bytes::from_static(b"event: ping\ndata: {\"type\": \"ping\"}\n\n"))];
                    Some((stream::iter(bytes), (body_stream, ctx, decoder, false, ping_interval, api_keys, key_id, usage_recorded, log_ctx, guard)))
                }
            }
        },
    )
    .flatten();

    initial_stream.chain(processing_stream)
}

/// 上下文窗口大小（200k tokens）
const CONTEXT_WINDOW_SIZE: i32 = 200_000;

/// 处理非流式请求
async fn handle_non_stream_request(
    provider: std::sync::Arc<crate::kiro::provider::KiroProvider>,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    auth_key_id: &str,
    request_body: &str,
    model: &str,
    input_tokens: i32,
    request_log: Option<std::sync::Arc<RequestLog>>,
    message_count: usize,
    start: Instant,
    log_request_body: String,
) -> Response {
    // 调用 Kiro API（支持多凭据故障转移）
    let response = match provider.call_api(request_body).await {
        Ok(resp) => resp,
        Err(e) => return map_provider_error(e),
    };

    handle_non_stream_response(
        response,
        api_keys,
        auth_key_id,
        model,
        input_tokens,
        request_log,
        message_count,
        start,
        log_request_body,
    )
    .await
}

/// 处理非流式请求（fixed 路由）
async fn handle_non_stream_request_fixed(
    provider: std::sync::Arc<crate::kiro::provider::KiroProvider>,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    auth_key_id: &str,
    request_body: &str,
    model: &str,
    input_tokens: i32,
    request_log: Option<std::sync::Arc<RequestLog>>,
    message_count: usize,
    start: Instant,
    log_request_body: String,
    credential_id: u64,
) -> Response {
    // 调用 Kiro API（fixed 路由：绑定指定凭据）
    let response = match provider.call_api_fixed(request_body, credential_id).await {
        Ok(resp) => resp,
        Err(e) => return map_provider_error(e),
    };

    handle_non_stream_response(
        response,
        api_keys,
        auth_key_id,
        model,
        input_tokens,
        request_log,
        message_count,
        start,
        log_request_body,
    )
    .await
}

/// 处理非流式响应体解析与日志记录
async fn handle_non_stream_response(
    response: reqwest::Response,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    auth_key_id: &str,
    model: &str,
    input_tokens: i32,
    request_log: Option<std::sync::Arc<RequestLog>>,
    message_count: usize,
    start: Instant,
    log_request_body: String,
) -> Response {
    // 读取响应体
    let body_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("读取响应体失败: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse::new(
                    "api_error",
                    format!("读取响应失败: {}", e),
                )),
            )
                .into_response();
        }
    };

    // 解析事件流
    let mut decoder = EventStreamDecoder::new();
    if let Err(e) = decoder.feed(&body_bytes) {
        tracing::warn!("缓冲区溢出: {}", e);
    }

    let mut text_content = String::new();
    let mut tool_uses: Vec<serde_json::Value> = Vec::new();
    let mut has_tool_use = false;
    let mut stop_reason = "end_turn".to_string();
    // 从 contextUsageEvent 计算的实际输入 tokens
    let mut context_input_tokens: Option<i32> = None;

    // 收集工具调用的增量 JSON
    let mut tool_json_buffers: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    for result in decoder.decode_iter() {
        match result {
            Ok(frame) => {
                if let Ok(event) = Event::from_frame(frame) {
                    match event {
                        Event::AssistantResponse(resp) => {
                            text_content.push_str(&resp.content);
                        }
                        Event::ToolUse(tool_use) => {
                            has_tool_use = true;

                            // 累积工具的 JSON 输入
                            let buffer = tool_json_buffers
                                .entry(tool_use.tool_use_id.clone())
                                .or_insert_with(String::new);
                            buffer.push_str(&tool_use.input);

                            // 如果是完整的工具调用，添加到列表
                            if tool_use.stop {
                                let input: serde_json::Value = if buffer.is_empty() {
                                    serde_json::json!({})
                                } else {
                                    serde_json::from_str(buffer).unwrap_or_else(|e| {
                                        tracing::warn!(
                                            "工具输入 JSON 解析失败: {}, tool_use_id: {}",
                                            e,
                                            tool_use.tool_use_id
                                        );
                                        serde_json::json!({})
                                    })
                                };

                                tool_uses.push(json!({
                                    "type": "tool_use",
                                    "id": tool_use.tool_use_id,
                                    "name": tool_use.name,
                                    "input": input
                                }));
                            }
                        }
                        Event::ContextUsage(context_usage) => {
                            // 从上下文使用百分比计算实际的 input_tokens
                            // 公式: percentage * 200000 / 100 = percentage * 2000
                            let actual_input_tokens = (context_usage.context_usage_percentage
                                * (CONTEXT_WINDOW_SIZE as f64)
                                / 100.0)
                                as i32;
                            context_input_tokens = Some(actual_input_tokens);
                            // 上下文使用量达到 100% 时，设置 stop_reason 为 model_context_window_exceeded
                            if context_usage.context_usage_percentage >= 100.0 {
                                stop_reason = "model_context_window_exceeded".to_string();
                            }
                            tracing::debug!(
                                "收到 contextUsageEvent: {}%, 计算 input_tokens: {}",
                                context_usage.context_usage_percentage,
                                actual_input_tokens
                            );
                        }
                        Event::Exception { exception_type, .. } => {
                            if exception_type == "ContentLengthExceededException" {
                                stop_reason = "max_tokens".to_string();
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                tracing::warn!("解码事件失败: {}", e);
            }
        }
    }

    // 确定 stop_reason
    if has_tool_use && stop_reason == "end_turn" {
        stop_reason = "tool_use".to_string();
    }

    // 构建响应内容
    let mut content: Vec<serde_json::Value> = Vec::new();

    if !text_content.is_empty() {
        content.push(json!({
            "type": "text",
            "text": text_content
        }));
    }

    content.extend(tool_uses);

    // 估算输出 tokens
    let output_tokens = token::estimate_output_tokens(&content);

    // 使用从 contextUsageEvent 计算的 input_tokens，如果没有则使用估算值
    let (token_source, final_input_tokens) = match context_input_tokens {
        Some(v) => ("upstream(contextUsageEvent)", v),
        None => ("local(estimate)", input_tokens),
    };
    tracing::info!(
        "token 统计 [非流式] [{}]: input={}, output={}",
        token_source, final_input_tokens, output_tokens
    );
    api_keys.record_usage(
        auth_key_id,
        final_input_tokens.max(0) as u64,
        output_tokens.max(0) as u64,
    );
    // 构建响应体用于日志记录
    let response_body = json!({
        "id": format!("msg_{}", Uuid::new_v4().to_string().replace('-', "")),
        "type": "message",
        "role": "assistant",
        "content": content,
        "model": model,
        "stop_reason": stop_reason,
        "stop_sequence": null,
        "usage": {
            "input_tokens": final_input_tokens,
            "output_tokens": output_tokens
        }
    });

    if let Some(log) = &request_log {
        log.push(RequestLogEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            model: model.to_string(),
            stream: false,
            message_count,
            input_tokens: final_input_tokens,
            output_tokens,
            token_source: token_source.to_string(),
            duration_ms: start.elapsed().as_millis() as u64,
            status: "success".to_string(),
            api_key_id: auth_key_id.to_string(),
            request_body: log_request_body.clone(),
            response_body: serde_json::to_string(&response_body).unwrap_or_default(),
        });
    }

    // 返回 Anthropic 标准 JSON 响应
    Json(response_body).into_response()
}

/// 检测模型名是否包含 "thinking" 后缀，若包含则覆写 thinking 配置
///
/// - Opus 4.6：覆写为 adaptive 类型
/// - 其他模型：覆写为 enabled 类型
/// - budget_tokens 固定为 20000
fn override_thinking_from_model_name(payload: &mut MessagesRequest) {
    let model_lower = payload.model.to_lowercase();
    if !model_lower.contains("thinking") {
        return;
    }

    let is_opus_4_6 = model_lower.contains("opus")
        && (model_lower.contains("4-6") || model_lower.contains("4.6"));

    let thinking_type = if is_opus_4_6 { "adaptive" } else { "enabled" };

    tracing::info!(
        model = %payload.model,
        thinking_type = thinking_type,
        "模型名包含 thinking 后缀，覆写 thinking 配置"
    );

    payload.thinking = Some(Thinking {
        thinking_type: thinking_type.to_string(),
        budget_tokens: 20000,
    });

    if is_opus_4_6 {
        payload.output_config = Some(OutputConfig {
            effort: "high".to_string(),
        });
    }
}

/// POST /v1/messages/count_tokens
///
/// 计算消息的 token 数量
pub async fn count_tokens(
    JsonExtractor(payload): JsonExtractor<CountTokensRequest>,
) -> impl IntoResponse {
    tracing::info!(
        model = %payload.model,
        message_count = %payload.messages.len(),
        "Received POST /v1/messages/count_tokens request"
    );

    let total_tokens = token::count_all_tokens(
        payload.model,
        payload.system,
        payload.messages,
        payload.tools,
    ) as i32;

    Json(CountTokensResponse {
        input_tokens: total_tokens.max(1) as i32,
    })
}

/// POST /cc/v1/messages
///
/// Claude Code 兼容端点，与 /v1/messages 的区别在于：
/// - 流式响应会等待 kiro 端返回 contextUsageEvent 后再发送 message_start
/// - message_start 中的 input_tokens 是从 contextUsageEvent 计算的准确值
pub async fn post_messages_cc(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthenticatedApiKey>,
    headers: HeaderMap,
    JsonExtractor(mut payload): JsonExtractor<MessagesRequest>,
) -> Response {
    tracing::info!(
        model = %payload.model,
        max_tokens = %payload.max_tokens,
        stream = %payload.stream,
        message_count = %payload.messages.len(),
        "Received POST /cc/v1/messages request"
    );

    // 检查 KiroProvider 是否可用
    let provider = match &state.kiro_provider {
        Some(p) => p.clone(),
        None => {
            tracing::error!("KiroProvider 未配置");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new(
                    "service_unavailable",
                    "Kiro API provider not configured",
                )),
            )
                .into_response();
        }
    };

    // 检测模型名是否包含 "thinking" 后缀，若包含则覆写 thinking 配置
    override_thinking_from_model_name(&mut payload);

    // 检查是否为 WebSearch 请求
    if websearch::has_web_search_tool(&payload) {
        tracing::info!("检测到 WebSearch 工具，路由到 WebSearch 处理");

        // 估算输入 tokens
        let input_tokens = token::count_all_tokens(
            payload.model.clone(),
            payload.system.clone(),
            payload.messages.clone(),
            payload.tools.clone(),
        ) as i32;

        // WebSearch 路径也执行路由决策，避免绕过 fixed/sticky
        let route = resolve_route(
            &auth,
            &headers,
            &state,
            provider.as_ref(),
            payload.metadata.as_ref(),
        );
        let fixed_credential_id = match route {
            RouteDecision::Fixed(credential_id) => Some(credential_id),
            RouteDecision::Sticky(identity) => state
                .sticky_tracker
                .as_ref()
                .and_then(|tracker| tracker.get_bound_credential(&identity.api_key)),
            RouteDecision::Global => None,
        };

        return websearch::handle_websearch_request(
            provider,
            &payload,
            input_tokens,
            fixed_credential_id,
        )
        .await;
    }

    // 转换请求
    let conversion_result = match convert_request(&payload) {
        Ok(result) => result,
        Err(e) => {
            let (error_type, message) = match &e {
                ConversionError::UnsupportedModel(model) => {
                    ("invalid_request_error", format!("模型不支持: {}", model))
                }
                ConversionError::EmptyMessages => {
                    ("invalid_request_error", "消息列表为空".to_string())
                }
            };
            tracing::warn!("请求转换失败: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(error_type, message)),
            )
                .into_response();
        }
    };

    // 构建 Kiro 请求
    let kiro_request = KiroRequest {
        conversation_state: conversion_result.conversation_state,
        profile_arn: state.profile_arn.clone(),
    };

    let request_body = match serde_json::to_string(&kiro_request) {
        Ok(body) => body,
        Err(e) => {
            tracing::error!("序列化请求失败: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    format!("序列化请求失败: {}", e),
                )),
            )
                .into_response();
        }
    };

    tracing::debug!("Kiro request body: {}", request_body);

    let message_count = payload.messages.len();
    let start = Instant::now();
    let log_request_body = if state.request_log.as_ref().is_some_and(|l| l.is_enabled()) {
        serde_json::to_string(&payload).unwrap_or_default()
    } else {
        String::new()
    };

    // 估算输入 tokens
    let input_tokens = token::count_all_tokens(
        payload.model.clone(),
        payload.system,
        payload.messages,
        payload.tools,
    ) as i32;

    // 检查是否启用了thinking
    let thinking_enabled = payload
        .thinking
        .as_ref()
        .map(|t| t.is_enabled())
        .unwrap_or(false);

    let route = resolve_route(
        &auth,
        &headers,
        &state,
        provider.as_ref(),
        payload.metadata.as_ref(),
    );

    match route {
        RouteDecision::Fixed(credential_id) => {
            if payload.stream {
                let response = match provider
                    .call_api_stream_fixed(&request_body, credential_id)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => return map_provider_error(e),
                };

                let ctx = BufferedStreamContext::new(&payload.model, input_tokens, thinking_enabled);
                let stream = create_buffered_sse_stream(
                    response,
                    ctx,
                    state.api_keys.clone(),
                    auth.key_id.clone(),
                    state.request_log.clone(),
                    payload.model.to_string(),
                    message_count,
                    start,
                    log_request_body,
                );

                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .header(header::CONNECTION, "keep-alive")
                    .body(Body::from_stream(stream))
                    .unwrap()
            } else {
                handle_non_stream_request_fixed(
                    provider,
                    state.api_keys.clone(),
                    &auth.key_id,
                    &request_body,
                    &payload.model,
                    input_tokens,
                    state.request_log.clone(),
                    message_count,
                    start,
                    log_request_body,
                    credential_id,
                )
                .await
            }
        }
        RouteDecision::Sticky(identity) => {
            if payload.stream {
                // Sticky 模式：通过 StickyTracker 选择凭据
                let (response, guard) = match provider
                    .call_api_stream_sticky(&request_body, &identity)
                    .await
                {
                    Ok(r) => r,
                    Err(e) => return map_sticky_error(e),
                };

                let ctx = BufferedStreamContext::new(&payload.model, input_tokens, thinking_enabled);
                let stream = create_buffered_sse_stream_with_guard(
                    response,
                    ctx,
                    state.api_keys.clone(),
                    auth.key_id.clone(),
                    state.request_log.clone(),
                    payload.model.to_string(),
                    message_count,
                    start,
                    log_request_body,
                    guard,
                );

                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "text/event-stream")
                    .header(header::CACHE_CONTROL, "no-cache")
                    .header(header::CONNECTION, "keep-alive")
                    .body(Body::from_stream(stream))
                    .unwrap()
            } else {
                // sticky 非流式：优先使用已绑定凭据，否则回退到 Global
                let bound_cred = state
                    .sticky_tracker
                    .as_ref()
                    .and_then(|tracker| tracker.get_bound_credential(&identity.api_key));
                if let Some(credential_id) = bound_cred {
                    handle_non_stream_request_fixed(
                        provider,
                        state.api_keys.clone(),
                        &auth.key_id,
                        &request_body,
                        &payload.model,
                        input_tokens,
                        state.request_log.clone(),
                        message_count,
                        start,
                        log_request_body,
                        credential_id,
                    )
                    .await
                } else {
                    handle_non_stream_request(
                        provider,
                        state.api_keys.clone(),
                        &auth.key_id,
                        &request_body,
                        &payload.model,
                        input_tokens,
                        state.request_log.clone(),
                        message_count,
                        start,
                        log_request_body,
                    )
                    .await
                }
            }
        }
        RouteDecision::Global => {
            if payload.stream {
                // 原有模式：缓冲流式响应
                handle_stream_request_buffered(
                    provider,
                    state.api_keys.clone(),
                    auth.key_id.clone(),
                    &request_body,
                    &payload.model,
                    input_tokens,
                    thinking_enabled,
                    state.request_log.clone(),
                    message_count,
                    start,
                    log_request_body,
                )
                .await
            } else {
                // 非流式响应（复用现有逻辑，已经使用正确的 input_tokens）
                handle_non_stream_request(
                    provider,
                    state.api_keys.clone(),
                    &auth.key_id,
                    &request_body,
                    &payload.model,
                    input_tokens,
                    state.request_log.clone(),
                    message_count,
                    start,
                    log_request_body,
                )
                .await
            }
        }
    }
}

/// 处理流式请求（缓冲版本）
///
/// 与 `handle_stream_request` 不同，此函数会缓冲所有事件直到流结束，
/// 然后用从 contextUsageEvent 计算的正确 input_tokens 生成 message_start 事件。
async fn handle_stream_request_buffered(
    provider: std::sync::Arc<crate::kiro::provider::KiroProvider>,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    key_id: String,
    request_body: &str,
    model: &str,
    estimated_input_tokens: i32,
    thinking_enabled: bool,
    request_log: Option<std::sync::Arc<RequestLog>>,
    message_count: usize,
    start: Instant,
    log_request_body: String,
) -> Response {
    // 调用 Kiro API（支持多凭据故障转移）
    let response = match provider.call_api_stream(request_body).await {
        Ok(resp) => resp,
        Err(e) => return map_provider_error(e),
    };

    // 创建缓冲流处理上下文
    let ctx = BufferedStreamContext::new(model, estimated_input_tokens, thinking_enabled);

    // 创建缓冲 SSE 流
    let stream = create_buffered_sse_stream(response, ctx, api_keys, key_id, request_log, model.to_string(), message_count, start, log_request_body);

    // 返回 SSE 流式响应
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap()
}

/// 创建缓冲 SSE 事件流
///
/// 工作流程：
/// 1. 等待上游流完成，期间只发送 ping 保活信号
/// 2. 使用 StreamContext 的事件处理逻辑处理所有 Kiro 事件，结果缓存
/// 3. 流结束后，用正确的 input_tokens 更正 message_start 事件
/// 4. 一次性发送所有事件
fn create_buffered_sse_stream(
    response: reqwest::Response,
    ctx: BufferedStreamContext,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    key_id: String,
    request_log: Option<std::sync::Arc<RequestLog>>,
    model: String,
    message_count: usize,
    start: Instant,
    log_request_body: String,
) -> impl Stream<Item = Result<Bytes, Infallible>> {
    let body_stream = response.bytes_stream();
    let log_ctx = StreamLogCtx { request_log, model, message_count, key_id: key_id.clone(), start, request_body: log_request_body, response_events: Vec::new() };

    stream::unfold(
        (
            body_stream,
            ctx,
            EventStreamDecoder::new(),
            false,
            interval(Duration::from_secs(PING_INTERVAL_SECS)),
            api_keys,
            key_id,
            log_ctx,
        ),
        |(mut body_stream, mut ctx, mut decoder, finished, mut ping_interval, api_keys, key_id, mut log_ctx)| async move {
            if finished {
                return None;
            }

            loop {
                tokio::select! {
                    // 使用 biased 模式，优先检查 ping 定时器
                    // 避免在上游 chunk 密集时 ping 被"饿死"
                    biased;

                    // 优先检查 ping 保活（等待期间发送空格保活）
                    _ = ping_interval.tick() => {
                        tracing::trace!("发送 ping 保活事件（缓冲模式）");
                        let bytes: Vec<Result<Bytes, Infallible>> = vec![Ok(Bytes::from_static(b"event: ping\ndata: {\"type\": \"ping\"}\n\n"))];
                        return Some((stream::iter(bytes), (body_stream, ctx, decoder, false, ping_interval, api_keys, key_id, log_ctx)));
                    }

                    // 然后处理数据流
                    chunk_result = body_stream.next() => {
                        match chunk_result {
                            Some(Ok(chunk)) => {
                                // 解码事件
                                if let Err(e) = decoder.feed(&chunk) {
                                    tracing::warn!("缓冲区溢出: {}", e);
                                }

                                for result in decoder.decode_iter() {
                                    match result {
                                        Ok(frame) => {
                                            if let Ok(event) = Event::from_frame(frame) {
                                                // 缓冲事件（复用 StreamContext 的处理逻辑）
                                                ctx.process_and_buffer(&event);
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!("解码事件失败: {}", e);
                                        }
                                    }
                                }
                                // 继续读取下一个 chunk，不发送任何数据
                            }
                            Some(Err(e)) => {
                                tracing::error!("读取响应流失败: {}", e);
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                let all_events = ctx.finish_and_get_all_events();
                                for se in &all_events {
                                    log_ctx.response_events.push(json!({
                                        "event": se.event,
                                        "data": se.data,
                                    }));
                                }
                                log_ctx.record(input, output, ctx.token_source(), &format!("error: {}", e));
                                let bytes = events_to_sse_bytes(all_events);
                                return Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, log_ctx)));
                            }
                            None => {
                                // 流结束，记录用量
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                let all_events = ctx.finish_and_get_all_events();
                                for se in &all_events {
                                    log_ctx.response_events.push(json!({
                                        "event": se.event,
                                        "data": se.data,
                                    }));
                                }
                                log_ctx.record(input, output, ctx.token_source(), "success");
                                let bytes = events_to_sse_bytes(all_events);
                                return Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, log_ctx)));
                            }
                        }
                    }
                }
            }
        },
    )
    .flatten()
}

/// 创建带 StreamGuard 的缓冲 SSE 事件流（sticky 模式用）
fn create_buffered_sse_stream_with_guard(
    response: reqwest::Response,
    ctx: BufferedStreamContext,
    api_keys: std::sync::Arc<crate::apikeys::ApiKeyManager>,
    key_id: String,
    request_log: Option<std::sync::Arc<RequestLog>>,
    model: String,
    message_count: usize,
    start: Instant,
    log_request_body: String,
    mut guard: StreamGuard,
) -> impl Stream<Item = Result<Bytes, Infallible>> {
    guard.activate();

    let body_stream = response.bytes_stream();
    let log_ctx = StreamLogCtx { request_log, model, message_count, key_id: key_id.clone(), start, request_body: log_request_body, response_events: Vec::new() };

    stream::unfold(
        (
            body_stream,
            ctx,
            EventStreamDecoder::new(),
            false,
            interval(Duration::from_secs(PING_INTERVAL_SECS)),
            api_keys,
            key_id,
            log_ctx,
            guard,
        ),
        |(mut body_stream, mut ctx, mut decoder, finished, mut ping_interval, api_keys, key_id, mut log_ctx, guard)| async move {
            if finished {
                return None;
            }

            loop {
                tokio::select! {
                    biased;

                    _ = ping_interval.tick() => {
                        tracing::trace!("发送 ping 保活事件（缓冲模式）");
                        let bytes: Vec<Result<Bytes, Infallible>> = vec![Ok(Bytes::from_static(b"event: ping\ndata: {\"type\": \"ping\"}\n\n"))];
                        return Some((stream::iter(bytes), (body_stream, ctx, decoder, false, ping_interval, api_keys, key_id, log_ctx, guard)));
                    }

                    chunk_result = body_stream.next() => {
                        match chunk_result {
                            Some(Ok(chunk)) => {
                                guard.touch();
                                if let Err(e) = decoder.feed(&chunk) {
                                    tracing::warn!("缓冲区溢出: {}", e);
                                }
                                for result in decoder.decode_iter() {
                                    match result {
                                        Ok(frame) => {
                                            if let Ok(event) = Event::from_frame(frame) {
                                                ctx.process_and_buffer(&event);
                                            }
                                        }
                                        Err(e) => { tracing::warn!("解码事件失败: {}", e); }
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                tracing::error!("读取响应流失败: {}", e);
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                let all_events = ctx.finish_and_get_all_events();
                                for se in &all_events {
                                    log_ctx.response_events.push(json!({
                                        "event": se.event,
                                        "data": se.data,
                                    }));
                                }
                                log_ctx.record(input, output, ctx.token_source(), &format!("error: {}", e));
                                let bytes = events_to_sse_bytes(all_events);
                                return Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, log_ctx, guard)));
                            }
                            None => {
                                let (input, output) = ctx.final_usage();
                                api_keys.record_usage(&key_id, input.max(0) as u64, output.max(0) as u64);
                                let all_events = ctx.finish_and_get_all_events();
                                for se in &all_events {
                                    log_ctx.response_events.push(json!({
                                        "event": se.event,
                                        "data": se.data,
                                    }));
                                }
                                log_ctx.record(input, output, ctx.token_source(), "success");
                                let bytes = events_to_sse_bytes(all_events);
                                return Some((stream::iter(bytes), (body_stream, ctx, decoder, true, ping_interval, api_keys, key_id, log_ctx, guard)));
                            }
                        }
                    }
                }
            }
        },
    )
    .flatten()
}

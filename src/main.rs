mod admin;
mod admin_ui;
mod anthropic;
mod apikeys;
mod common;
mod http_client;
mod kiro;
mod kiro_oauth_web;
mod model;
mod sticky;
pub mod request_log;
pub mod token;

use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use kiro::model::credentials::{CredentialsConfig, KiroCredentials};
use kiro::provider::KiroProvider;
use kiro::token_manager::MultiTokenManager;
use model::arg::Args;
use model::config::Config;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config_path = args
        .config
        .unwrap_or_else(|| Config::default_config_path().to_string());
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        tracing::error!("加载配置失败: {}", e);
        std::process::exit(1);
    });

    let credentials_path = args
        .credentials
        .unwrap_or_else(|| KiroCredentials::default_credentials_path().to_string());
    let credentials_config = CredentialsConfig::load(&credentials_path).unwrap_or_else(|e| {
        tracing::error!("加载凭证失败: {}", e);
        std::process::exit(1);
    });

    let is_multiple_format = credentials_config.is_multiple();
    let credentials_list = credentials_config.into_sorted_credentials();
    tracing::info!("已加载 {} 个凭据配置", credentials_list.len());

    let first_credentials = credentials_list.first().cloned().unwrap_or_default();

    let api_key = config.api_key.clone().unwrap_or_else(|| {
        tracing::error!("配置文件中未设置 apiKey");
        std::process::exit(1);
    });

    let api_key_store = Path::new(&config_path)
        .parent()
        .map(|p| p.join("api_keys.db"));
    let api_keys = Arc::new(apikeys::ApiKeyManager::new(api_key.clone(), api_key_store));
    let request_log = Arc::new(request_log::RequestLog::new());

    let proxy_config = config.proxy_url.as_ref().map(|url| {
        let mut proxy = http_client::ProxyConfig::new(url);
        if let (Some(username), Some(password)) = (&config.proxy_username, &config.proxy_password) {
            proxy = proxy.with_auth(username, password);
        }
        proxy
    });

    let token_manager = MultiTokenManager::new(
        config.clone(),
        credentials_list,
        proxy_config.clone(),
        Some(credentials_path.into()),
        is_multiple_format,
    )
    .unwrap_or_else(|e| {
        tracing::error!("创建 Token 管理器失败: {}", e);
        std::process::exit(1);
    });
    let token_manager = Arc::new(token_manager);
    let mut kiro_provider = KiroProvider::with_proxy(token_manager.clone(), proxy_config.clone());

    // 始终创建 StickyTracker（支持运行时切换到 sticky 模式）
    let sticky_tracker = Arc::new(sticky::StickyTracker::new(
        config.max_concurrent_per_credential,
        config.max_concurrent_per_key,
        config.sticky_expiry_minutes,
        config.zombie_stream_timeout_minutes,
    ));
    kiro_provider = kiro_provider.with_sticky_tracker(sticky_tracker.clone());

    // 后台清理任务（始终运行，非 sticky 模式下无数据不会有实际开销）
    let cleanup_tracker = sticky_tracker.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let zombies = cleanup_tracker.cleanup_zombies();
            let expired = cleanup_tracker.cleanup_expired_bindings();
            if zombies > 0 || expired > 0 {
                tracing::info!(
                    "Sticky 清理: 移除 {} 个僵尸流, {} 个过期绑定",
                    zombies,
                    expired
                );
            }
        }
    });

    if config.load_balancing_mode == "sticky" {
        tracing::info!(
            "Sticky 负载均衡已启用 (max_per_credential={}, max_per_key={}, expiry={}min, zombie={}min)",
            config.max_concurrent_per_credential,
            config.max_concurrent_per_key,
            config.sticky_expiry_minutes,
            config.zombie_stream_timeout_minutes,
        );
    }

    token::init_config(token::CountTokensConfig {
        api_url: config.count_tokens_api_url.clone(),
        api_key: config.count_tokens_api_key.clone(),
        auth_type: config.count_tokens_auth_type.clone(),
        proxy: proxy_config,
        tls_backend: config.tls_backend,
    });

    let anthropic_app = anthropic::create_router_with_provider(
        api_keys.clone(),
        Some(kiro_provider),
        first_credentials.profile_arn.clone(),
        Some(request_log.clone()),
        Some(sticky_tracker.clone()),
    );

    let admin_enabled = config
        .admin_api_key
        .as_ref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
        || config
            .admin_password
            .as_ref()
            .map(|p| !p.trim().is_empty())
            .unwrap_or(false);

    let app = if admin_enabled {
        let mut admin_service = admin::AdminService::new(token_manager.clone(), api_keys.clone(), Some(request_log.clone()));
        admin_service = admin_service.with_sticky_tracker(sticky_tracker.clone());

        let admin_username = config
            .admin_username
            .clone()
            .unwrap_or_else(|| "admin".to_string());
        let admin_password = config
            .admin_password
            .clone()
            .unwrap_or_else(|| "admin".to_string());

        let admin_state = admin::AdminState::new(admin_username, admin_password, admin_service);
        let admin_app = admin::create_admin_router(admin_state.clone());
        let admin_ui_app = admin_ui::create_admin_ui_router();
        let oauth_web_app =
            kiro_oauth_web::create_kiro_oauth_router(admin_state.clone(), config.clone());

        anthropic_app
            .nest("/api/admin", admin_app)
            .nest("/admin", admin_ui_app.clone())
            .fallback_service(admin_ui_app)
            .nest("/v0/oauth/kiro", oauth_web_app)
    } else {
        anthropic_app
    };

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("启动服务: {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

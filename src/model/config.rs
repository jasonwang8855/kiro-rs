use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsBackend {
    Rustls,
    NativeTls,
}

impl Default for TlsBackend {
    fn default() -> Self {
        Self::Rustls
    }
}

/// KNA 搴旂敤閰嶇疆
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_region")]
    pub region: String,

    /// Auth Region锛堢敤浜?Token 鍒锋柊锛夛紝鏈厤缃椂鍥為€€鍒?region
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_region: Option<String>,

    /// API Region锛堢敤浜?API 璇锋眰锛夛紝鏈厤缃椂鍥為€€鍒?region
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_region: Option<String>,

    #[serde(default = "default_kiro_version")]
    pub kiro_version: String,

    #[serde(default)]
    pub machine_id: Option<String>,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default = "default_system_version")]
    pub system_version: String,

    #[serde(default = "default_node_version")]
    pub node_version: String,

    #[serde(default = "default_tls_backend")]
    pub tls_backend: TlsBackend,

    /// 澶栭儴 count_tokens API 鍦板潃锛堝彲閫夛級
    #[serde(default)]
    pub count_tokens_api_url: Option<String>,

    /// count_tokens API 瀵嗛挜锛堝彲閫夛級
    #[serde(default)]
    pub count_tokens_api_key: Option<String>,

    /// count_tokens API 璁よ瘉绫诲瀷锛堝彲閫夛紝"x-api-key" 鎴?"bearer"锛岄粯璁?"x-api-key"锛?
    #[serde(default = "default_count_tokens_auth_type")]
    pub count_tokens_auth_type: String,

    /// HTTP 浠ｇ悊鍦板潃锛堝彲閫夛級
    /// 鏀寔鏍煎紡: http://host:port, https://host:port, socks5://host:port
    #[serde(default)]
    pub proxy_url: Option<String>,

    /// 浠ｇ悊璁よ瘉鐢ㄦ埛鍚嶏紙鍙€夛級
    #[serde(default)]
    pub proxy_username: Option<String>,

    /// 浠ｇ悊璁よ瘉瀵嗙爜锛堝彲閫夛級
    #[serde(default)]
    pub proxy_password: Option<String>,

    /// Admin API 瀵嗛挜锛堝彲閫夛紝鍚敤 Admin API 鍔熻兘锛?
    #[serde(default)]
    pub admin_api_key: Option<String>,

    #[serde(default)]
    pub admin_username: Option<String>,

    #[serde(default)]
    pub admin_password: Option<String>,

    /// 璐熻浇鍧囪　妯″紡锛?priority" 鎴?"balanced"锛?
    #[serde(default = "default_load_balancing_mode")]
    pub load_balancing_mode: String,

    #[serde(default = "default_max_concurrent_per_credential")]
    pub max_concurrent_per_credential: u32,

    #[serde(default = "default_max_concurrent_per_key")]
    pub max_concurrent_per_key: u32,

    #[serde(default = "default_sticky_expiry_minutes")]
    pub sticky_expiry_minutes: u32,

    #[serde(default = "default_zombie_stream_timeout_minutes")]
    pub zombie_stream_timeout_minutes: u32,

    /// 閰嶇疆鏂囦欢璺緞锛堣繍琛屾椂鍏冩暟鎹紝涓嶅啓鍏?JSON锛?
    #[serde(skip)]
    config_path: Option<PathBuf>,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_region() -> String {
    "us-east-1".to_string()
}

fn default_kiro_version() -> String {
    "0.10.0".to_string()
}

fn default_system_version() -> String {
    const SYSTEM_VERSIONS: &[&str] = &["darwin#24.6.0", "win32#10.0.22631"];
    SYSTEM_VERSIONS[fastrand::usize(..SYSTEM_VERSIONS.len())].to_string()
}

fn default_node_version() -> String {
    "22.21.1".to_string()
}

fn default_count_tokens_auth_type() -> String {
    "x-api-key".to_string()
}

fn default_tls_backend() -> TlsBackend {
    TlsBackend::Rustls
}

fn default_load_balancing_mode() -> String {
    "priority".to_string()
}

fn default_max_concurrent_per_credential() -> u32 {
    2
}

fn default_max_concurrent_per_key() -> u32 {
    5
}

fn default_sticky_expiry_minutes() -> u32 {
    30
}

fn default_zombie_stream_timeout_minutes() -> u32 {
    15
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            region: default_region(),
            auth_region: None,
            api_region: None,
            kiro_version: default_kiro_version(),
            machine_id: None,
            api_key: None,
            system_version: default_system_version(),
            node_version: default_node_version(),
            tls_backend: default_tls_backend(),
            count_tokens_api_url: None,
            count_tokens_api_key: None,
            count_tokens_auth_type: default_count_tokens_auth_type(),
            proxy_url: None,
            proxy_username: None,
            proxy_password: None,
            admin_api_key: None,
            admin_username: None,
            admin_password: None,
            load_balancing_mode: default_load_balancing_mode(),
            max_concurrent_per_credential: default_max_concurrent_per_credential(),
            max_concurrent_per_key: default_max_concurrent_per_key(),
            sticky_expiry_minutes: default_sticky_expiry_minutes(),
            zombie_stream_timeout_minutes: default_zombie_stream_timeout_minutes(),
            config_path: None,
        }
    }
}

impl Config {
    /// 鑾峰彇榛樿閰嶇疆鏂囦欢璺緞
    pub fn default_config_path() -> &'static str {
        "config.json"
    }

    /// 鑾峰彇鏈夋晥鐨?Auth Region锛堢敤浜?Token 鍒锋柊锛?
    /// 浼樺厛浣跨敤 auth_region锛屾湭閰嶇疆鏃跺洖閫€鍒?region
    pub fn effective_auth_region(&self) -> &str {
        self.auth_region.as_deref().unwrap_or(&self.region)
    }

    /// 鑾峰彇鏈夋晥鐨?API Region锛堢敤浜?API 璇锋眰锛?
    /// 浼樺厛浣跨敤 api_region锛屾湭閰嶇疆鏃跺洖閫€鍒?region
    pub fn effective_api_region(&self) -> &str {
        self.api_region.as_deref().unwrap_or(&self.region)
    }

    /// 浠庢枃浠跺姞杞介厤缃?
    pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            // 閰嶇疆鏂囦欢涓嶅瓨鍦紝杩斿洖榛樿閰嶇疆
            let mut config = Self::default();
            config.config_path = Some(path.to_path_buf());
            return Ok(config);
        }

        let content = fs::read_to_string(path)?;
        let mut config: Config = serde_json::from_str(&content)?;
        config.config_path = Some(path.to_path_buf());
        Ok(config)
    }

    /// 鑾峰彇閰嶇疆鏂囦欢璺緞锛堝鏋滄湁锛?
    pub fn config_path(&self) -> Option<&Path> {
        self.config_path.as_deref()
    }

    /// 灏嗗綋鍓嶉厤缃啓鍥炲師濮嬮厤缃枃浠?
    pub fn save(&self) -> anyhow::Result<()> {
        let path = self
            .config_path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Config path is unknown, cannot save config"))?;

        let content = serde_json::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;
        Ok(())
    }
}

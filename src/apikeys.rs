use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::auth;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyRecord {
    pub id: String,
    pub name: String,
    pub key: String,
    pub enabled: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub request_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyPublicInfo {
    pub id: String,
    pub name: String,
    pub key: String,
    pub enabled: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub request_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub key_preview: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyUsageOverview {
    pub total_keys: usize,
    pub enabled_keys: usize,
    pub total_requests: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

#[derive(Debug, Clone)]
pub struct AuthenticatedApiKey {
    pub key_id: String,
}

pub struct ApiKeyManager {
    keys: Mutex<Vec<ApiKeyRecord>>,
    store_path: Option<PathBuf>,
}

impl ApiKeyManager {
    pub fn new(initial_key: String, store_path: Option<PathBuf>) -> Self {
        let mut keys = Self::load_from(&store_path).unwrap_or_default();

        if keys.is_empty() {
            keys.push(ApiKeyRecord {
                id: Uuid::new_v4().to_string(),
                name: "Default".to_string(),
                key: initial_key,
                enabled: true,
                created_at: Utc::now().to_rfc3339(),
                last_used_at: None,
                request_count: 0,
                input_tokens: 0,
                output_tokens: 0,
            });
        } else if !initial_key.trim().is_empty()
            && !keys
                .iter()
                .any(|k| auth::constant_time_eq(k.key.as_str(), initial_key.as_str()))
        {
            keys.push(ApiKeyRecord {
                id: Uuid::new_v4().to_string(),
                name: "Config API Key".to_string(),
                key: initial_key,
                enabled: true,
                created_at: Utc::now().to_rfc3339(),
                last_used_at: None,
                request_count: 0,
                input_tokens: 0,
                output_tokens: 0,
            });
        }

        let manager = Self {
            keys: Mutex::new(keys),
            store_path,
        };
        manager.save_to_disk();
        manager
    }

    pub fn authenticate(&self, incoming: &str) -> Option<AuthenticatedApiKey> {
        let mut keys = self.keys.lock();
        let now = Utc::now().to_rfc3339();
        for item in keys.iter_mut().filter(|k| k.enabled) {
            if auth::constant_time_eq(item.key.as_str(), incoming) {
                item.last_used_at = Some(now);
                return Some(AuthenticatedApiKey {
                    key_id: item.id.clone(),
                });
            }
        }
        None
    }

    pub fn record_usage(&self, key_id: &str, input_tokens: u64, output_tokens: u64) {
        let mut keys = self.keys.lock();
        if let Some(item) = keys.iter_mut().find(|k| k.id == key_id) {
            item.request_count = item.request_count.saturating_add(1);
            item.input_tokens = item.input_tokens.saturating_add(input_tokens);
            item.output_tokens = item.output_tokens.saturating_add(output_tokens);
            item.last_used_at = Some(Utc::now().to_rfc3339());
            drop(keys);
            self.save_to_disk();
        }
    }

    pub fn list(&self) -> Vec<ApiKeyPublicInfo> {
        self.keys
            .lock()
            .iter()
            .map(|k| ApiKeyPublicInfo {
                id: k.id.clone(),
                name: k.name.clone(),
                key: k.key.clone(),
                enabled: k.enabled,
                created_at: k.created_at.clone(),
                last_used_at: k.last_used_at.clone(),
                request_count: k.request_count,
                input_tokens: k.input_tokens,
                output_tokens: k.output_tokens,
                key_preview: preview_key(&k.key),
            })
            .collect()
    }

    pub fn overview(&self) -> ApiKeyUsageOverview {
        let keys = self.keys.lock();
        ApiKeyUsageOverview {
            total_keys: keys.len(),
            enabled_keys: keys.iter().filter(|k| k.enabled).count(),
            total_requests: keys.iter().map(|k| k.request_count).sum(),
            total_input_tokens: keys.iter().map(|k| k.input_tokens).sum(),
            total_output_tokens: keys.iter().map(|k| k.output_tokens).sum(),
        }
    }

    pub fn create_key(&self, name: String) -> ApiKeyRecord {
        let raw = format!("sk-kiro-rs-{}", Uuid::new_v4().simple());
        let item = ApiKeyRecord {
            id: Uuid::new_v4().to_string(),
            name,
            key: raw,
            enabled: true,
            created_at: Utc::now().to_rfc3339(),
            last_used_at: None,
            request_count: 0,
            input_tokens: 0,
            output_tokens: 0,
        };
        self.keys.lock().push(item.clone());
        self.save_to_disk();
        item
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> bool {
        let mut keys = self.keys.lock();
        if let Some(item) = keys.iter_mut().find(|k| k.id == id) {
            item.enabled = enabled;
            drop(keys);
            self.save_to_disk();
            return true;
        }
        false
    }

    pub fn delete_key(&self, id: &str) -> bool {
        let mut keys = self.keys.lock();
        let original = keys.len();
        keys.retain(|k| k.id != id);
        let changed = keys.len() != original;
        drop(keys);
        if changed {
            self.save_to_disk();
        }
        changed
    }

    fn load_from(path: &Option<PathBuf>) -> Option<Vec<ApiKeyRecord>> {
        let p = path.as_ref()?;
        let content = fs::read_to_string(p).ok()?;
        serde_json::from_str::<Vec<ApiKeyRecord>>(&content).ok()
    }

    fn save_to_disk(&self) {
        let path = match &self.store_path {
            Some(p) => p,
            None => return,
        };

        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::warn!("创建 API Key 存储目录失败: {}", e);
                return;
            }
        }

        let content = match serde_json::to_string_pretty(&*self.keys.lock()) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("序列化 API Key 失败: {}", e);
                return;
            }
        };

        if let Err(e) = fs::write(path, content) {
            tracing::warn!("写入 API Key 文件失败: {}", e);
        }
    }
}

fn preview_key(raw: &str) -> String {
    let len = raw.len();
    if len <= 8 {
        return "********".to_string();
    }
    format!("{}****{}", &raw[..4], &raw[len.saturating_sub(4)..])
}

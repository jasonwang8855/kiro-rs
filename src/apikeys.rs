use std::fs;
use std::path::PathBuf;

use chrono::Utc;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::common::auth;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RoutingMode {
    Auto,
    Fixed,
}

impl Default for RoutingMode {
    fn default() -> Self {
        Self::Auto
    }
}

impl RoutingMode {
    fn as_db_value(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Fixed => "fixed",
        }
    }

    fn from_db_value(value: &str) -> Self {
        if value.eq_ignore_ascii_case("fixed") {
            Self::Fixed
        } else {
            Self::Auto
        }
    }
}

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
    #[serde(default)]
    pub routing_mode: RoutingMode,
    #[serde(default)]
    pub credential_id: Option<u64>,
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
    pub routing_mode: RoutingMode,
    pub credential_id: Option<u64>,
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
    pub routing_mode: RoutingMode,
    pub credential_id: Option<u64>,
}

pub struct ApiKeyManager {
    conn: Mutex<Connection>,
}

impl ApiKeyManager {
    pub fn new(initial_key: String, store_path: Option<PathBuf>) -> Self {
        let conn = match &store_path {
            Some(p) => {
                if let Some(parent) = p.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                Connection::open(p).expect("无法打开 SQLite 数据库")
            }
            None => Connection::open_in_memory().expect("无法创建内存数据库"),
        };

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .expect("设置 PRAGMA 失败");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                key TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                last_used_at TEXT,
                request_count INTEGER NOT NULL DEFAULT 0,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )
        .expect("建表失败");

        // 幂等迁移：新增 routing_mode / credential_id 字段
        let columns = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(api_keys)")
                .expect("读取表结构失败");
            stmt.query_map([], |row| row.get::<_, String>(1))
                .expect("读取表结构失败")
                .filter_map(|r| r.ok())
                .collect::<Vec<String>>()
        };

        if !columns.iter().any(|c| c == "routing_mode") {
            let _ = conn.execute(
                "ALTER TABLE api_keys ADD COLUMN routing_mode TEXT NOT NULL DEFAULT 'auto'",
                [],
            );
        }
        if !columns.iter().any(|c| c == "credential_id") {
            let _ = conn.execute("ALTER TABLE api_keys ADD COLUMN credential_id INTEGER", []);
        }

        // 自动迁移旧 JSON 文件
        if let Some(db_path) = &store_path {
            let json_path = db_path.with_extension("json");
            if json_path.exists() {
                if let Ok(content) = fs::read_to_string(&json_path) {
                    if let Ok(records) = serde_json::from_str::<Vec<ApiKeyRecord>>(&content) {
                        for r in &records {
                            let _ = conn.execute(
                                "INSERT OR IGNORE INTO api_keys (id, name, key, enabled, created_at, last_used_at, request_count, input_tokens, output_tokens, routing_mode, credential_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                                params![
                                    r.id,
                                    r.name,
                                    r.key,
                                    r.enabled as i32,
                                    r.created_at,
                                    r.last_used_at,
                                    r.request_count as i64,
                                    r.input_tokens as i64,
                                    r.output_tokens as i64,
                                    r.routing_mode.as_db_value(),
                                    r.credential_id.and_then(|id| i64::try_from(id).ok())
                                ],
                            );
                        }
                        let migrated = json_path.with_extension("json.migrated");
                        let _ = fs::rename(&json_path, &migrated);
                        tracing::info!("已从 JSON 迁移 {} 条 API Key 到 SQLite", records.len());
                    }
                }
            }
        }

        let manager = Self { conn: Mutex::new(conn) };

        // 确保 initial_key 存在
        let count: i64 = manager.conn.lock()
            .query_row("SELECT COUNT(*) FROM api_keys", [], |row| row.get(0))
            .unwrap_or(0);

        if count == 0 {
            let _ = manager.conn.lock().execute(
                "INSERT INTO api_keys (id, name, key, enabled, created_at, request_count, input_tokens, output_tokens, routing_mode, credential_id) VALUES (?1,?2,?3,1,?4,0,0,0,?5,NULL)",
                params![
                    Uuid::new_v4().to_string(),
                    "Default",
                    initial_key,
                    Utc::now().to_rfc3339(),
                    RoutingMode::Auto.as_db_value()
                ],
            );
        } else if !initial_key.trim().is_empty() {
            // 检查 initial_key 是否已存在（常量时间比较）
            let keys: Vec<String> = {
                let conn = manager.conn.lock();
                let mut stmt = conn.prepare("SELECT key FROM api_keys").unwrap();
                stmt.query_map([], |row| row.get::<_, String>(0))
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .collect()
            };
            if !keys.iter().any(|k| auth::constant_time_eq(k.as_str(), initial_key.as_str())) {
                let _ = manager.conn.lock().execute(
                    "INSERT INTO api_keys (id, name, key, enabled, created_at, request_count, input_tokens, output_tokens, routing_mode, credential_id) VALUES (?1,?2,?3,1,?4,0,0,0,?5,NULL)",
                    params![
                        Uuid::new_v4().to_string(),
                        "Config API Key",
                        initial_key,
                        Utc::now().to_rfc3339(),
                        RoutingMode::Auto.as_db_value()
                    ],
                );
            }
        }

        manager
    }

    pub fn authenticate(&self, incoming: &str) -> Option<AuthenticatedApiKey> {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        let mut stmt = conn
            .prepare("SELECT id, key, routing_mode, credential_id FROM api_keys WHERE enabled = 1")
            .ok()?;
        let rows: Vec<(String, String, RoutingMode, Option<u64>)> = stmt
            .query_map([], |row| {
                let routing_mode_raw: String = row.get(2)?;
                let credential_id_raw: Option<i64> = row.get(3)?;
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    RoutingMode::from_db_value(&routing_mode_raw),
                    credential_id_raw.and_then(|id| u64::try_from(id).ok()),
                ))
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();

        for (id, key, routing_mode, credential_id) in rows {
            if auth::constant_time_eq(key.as_str(), incoming) {
                let _ = conn.execute(
                    "UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2",
                    params![now, &id],
                );
                return Some(AuthenticatedApiKey {
                    key_id: id,
                    routing_mode,
                    credential_id,
                });
            }
        }
        None
    }

    pub fn record_usage(&self, key_id: &str, input_tokens: u64, output_tokens: u64) {
        let conn = self.conn.lock();
        let now = Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE api_keys SET request_count = request_count + 1, input_tokens = input_tokens + ?1, output_tokens = output_tokens + ?2, last_used_at = ?3 WHERE id = ?4",
            params![input_tokens as i64, output_tokens as i64, now, key_id],
        );
    }

    pub fn list(&self) -> Vec<ApiKeyPublicInfo> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT id, name, key, enabled, created_at, last_used_at, request_count, input_tokens, output_tokens, routing_mode, credential_id FROM api_keys")
            .unwrap();
        stmt.query_map([], |row| {
            let key: String = row.get(2)?;
            let routing_mode_raw: String = row.get(9)?;
            let credential_id_raw: Option<i64> = row.get(10)?;
            Ok(ApiKeyPublicInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                key: key.clone(),
                enabled: row.get::<_, i32>(3)? != 0,
                created_at: row.get(4)?,
                last_used_at: row.get(5)?,
                request_count: row.get::<_, i64>(6)? as u64,
                input_tokens: row.get::<_, i64>(7)? as u64,
                output_tokens: row.get::<_, i64>(8)? as u64,
                key_preview: preview_key(&key),
                routing_mode: RoutingMode::from_db_value(&routing_mode_raw),
                credential_id: credential_id_raw.and_then(|id| u64::try_from(id).ok()),
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    pub fn overview(&self) -> ApiKeyUsageOverview {
        let conn = self.conn.lock();
        let (total, enabled, requests, input, output) = conn
            .query_row(
                "SELECT COUNT(*), SUM(CASE WHEN enabled=1 THEN 1 ELSE 0 END), COALESCE(SUM(request_count),0), COALESCE(SUM(input_tokens),0), COALESCE(SUM(output_tokens),0) FROM api_keys",
                [],
                |row| Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, i64>(2)? as u64,
                    row.get::<_, i64>(3)? as u64,
                    row.get::<_, i64>(4)? as u64,
                )),
            )
            .unwrap_or((0, 0, 0, 0, 0));
        ApiKeyUsageOverview {
            total_keys: total,
            enabled_keys: enabled,
            total_requests: requests,
            total_input_tokens: input,
            total_output_tokens: output,
        }
    }

    pub fn create_key(
        &self,
        name: String,
        routing_mode: RoutingMode,
        credential_id: Option<u64>,
    ) -> Result<ApiKeyRecord, rusqlite::Error> {
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
            routing_mode,
            credential_id,
        };
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO api_keys (id, name, key, enabled, created_at, request_count, input_tokens, output_tokens, routing_mode, credential_id) VALUES (?1,?2,?3,1,?4,0,0,0,?5,?6)",
            params![
                &item.id,
                &item.name,
                &item.key,
                &item.created_at,
                item.routing_mode.as_db_value(),
                item.credential_id.and_then(|id| i64::try_from(id).ok())
            ],
        )?;
        Ok(item)
    }

    pub fn set_routing(
        &self,
        id: &str,
        routing_mode: RoutingMode,
        credential_id: Option<u64>,
    ) -> bool {
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "UPDATE api_keys SET routing_mode = ?1, credential_id = ?2 WHERE id = ?3",
                params![
                    routing_mode.as_db_value(),
                    credential_id.and_then(|id| i64::try_from(id).ok()),
                    id
                ],
            )
            .unwrap_or(0);
        changed > 0
    }

    /// 将所有引用指定凭据的 API key 重置为 Auto 模式
    pub fn reset_routing_for_credential(&self, credential_id: u64) {
        let Ok(credential_id_i64) = i64::try_from(credential_id) else {
            return;
        };
        let conn = self.conn.lock();
        let _ = conn.execute(
            "UPDATE api_keys SET routing_mode = 'auto', credential_id = NULL WHERE credential_id = ?1",
            params![credential_id_i64],
        );
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> bool {
        let conn = self.conn.lock();
        let changed = conn
            .execute(
                "UPDATE api_keys SET enabled = ?1 WHERE id = ?2",
                params![enabled as i32, id],
            )
            .unwrap_or(0);
        changed > 0
    }

    pub fn delete_key(&self, id: &str) -> bool {
        let conn = self.conn.lock();
        let changed = conn
            .execute("DELETE FROM api_keys WHERE id = ?1", params![id])
            .unwrap_or(0);
        changed > 0
    }
}

fn preview_key(raw: &str) -> String {
    let len = raw.len();
    if len <= 8 {
        return "********".to_string();
    }
    format!("{}****{}", &raw[..4], &raw[len.saturating_sub(4)..])
}

use std::collections::VecDeque;

use parking_lot::Mutex;
use serde::Serialize;

const MAX_LOG_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLogEntry {
    pub id: String,
    pub timestamp: String,
    pub model: String,
    pub stream: bool,
    pub message_count: usize,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub token_source: String,
    pub duration_ms: u64,
    pub status: String,
    pub api_key_id: String,
    pub request_body: String,
    pub response_body: String,
}

pub struct RequestLog {
    entries: Mutex<VecDeque<RequestLogEntry>>,
}

impl RequestLog {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES)),
        }
    }

    pub fn push(&self, entry: RequestLogEntry) {
        let mut entries = self.entries.lock();
        if entries.len() >= MAX_LOG_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    pub fn entries_since(&self, since_id: Option<&str>) -> Vec<RequestLogEntry> {
        let entries = self.entries.lock();
        match since_id {
            Some(id) => {
                if let Some(pos) = entries.iter().position(|e| e.id == id) {
                    entries.iter().skip(pos + 1).cloned().collect()
                } else {
                    entries.iter().cloned().collect()
                }
            }
            None => entries.iter().cloned().collect(),
        }
    }
}

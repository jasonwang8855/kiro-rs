use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RequestIdentity {
    pub api_key: String,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone)]
struct StickyBinding {
    credential_id: u64,
    last_request_at: Instant,
}

#[derive(Debug, Default)]
struct CredentialConcurrency {
    active_count: u32,
    per_key_counts: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
struct ActiveStream {
    credential_id: u64,
    api_key: String,
    activated: bool,
    last_touch_at: Instant,
    session_id: Option<String>,
}

pub type StreamId = u64;

#[derive(Debug, Clone)]
pub enum AcquireResult {
    Acquired {
        credential_id: u64,
        stream_id: StreamId,
    },
    AllFull {
        retry_after_secs: f64,
    },
}

#[derive(Debug, Clone)]
pub struct AvailableCredential {
    pub id: u64,
    pub supports_opus: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StickyStats {
    pub hits: u64,
    pub assignments: u64,
    pub unbinds: u64,
    pub queue_jumps: u64,
    pub rejections_429: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StickySnapshot {
    pub credentials: Vec<CredentialSnapshot>,
    pub stats: StickyStats,
    pub active_stream_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSnapshot {
    pub id: u64,
    pub active_count: u32,
    pub bound_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamSnapshot {
    pub stream_id: StreamId,
    pub credential_id: u64,
    pub api_key: String,
    pub session_id: Option<String>,
    pub activated: bool,
    pub started_at_secs_ago: u64,
}

pub struct StickyTracker {
    bindings: Mutex<HashMap<String, StickyBinding>>,
    concurrency: Mutex<HashMap<u64, CredentialConcurrency>>,
    active_streams: Mutex<HashMap<StreamId, ActiveStream>>,
    stats: Mutex<StickyStats>,
    next_stream_id: AtomicU64,
    max_concurrent_per_credential: u32,
    max_concurrent_per_key: u32,
    sticky_expiry_minutes: u32,
    zombie_stream_timeout_minutes: u32,
}

impl StickyTracker {
    pub fn new(
        max_concurrent_per_credential: u32,
        max_concurrent_per_key: u32,
        sticky_expiry_minutes: u32,
        zombie_stream_timeout_minutes: u32,
    ) -> Self {
        Self {
            bindings: Mutex::new(HashMap::new()),
            concurrency: Mutex::new(HashMap::new()),
            active_streams: Mutex::new(HashMap::new()),
            stats: Mutex::new(StickyStats::default()),
            next_stream_id: AtomicU64::new(1),
            max_concurrent_per_credential,
            max_concurrent_per_key,
            sticky_expiry_minutes,
            zombie_stream_timeout_minutes,
        }
    }

    pub fn try_acquire(
        &self,
        identity: &RequestIdentity,
        available_credentials: &[AvailableCredential],
    ) -> AcquireResult {
        let now = Instant::now();
        let api_key = identity.api_key.clone();

        // 锁顺序: bindings → concurrency → active_streams
        let bound_credential = {
            let bindings = self.bindings.lock();
            bindings.get(&api_key).map(|binding| binding.credential_id)
        };

        // 持有 concurrency 锁贯穿 check+reserve 过程，防止并发竞态超额分配
        let mut concurrency = self.concurrency.lock();

        if let Some(credential_id) = bound_credential {
            let is_still_available = available_credentials
                .iter()
                .any(|credential| credential.id == credential_id);
            if !is_still_available {
                drop(concurrency);
                if self.remove_binding_if_matches(&api_key, credential_id) {
                    let mut stats = self.stats.lock();
                    stats.unbinds = stats.unbinds.saturating_add(1);
                }
                // 重新获取 concurrency 锁进入 select_best 路径
                concurrency = self.concurrency.lock();
            } else {
                let (active_count, per_key_count) =
                    Self::current_counts_locked(&concurrency, credential_id, &api_key);
                let is_full = active_count >= self.max_concurrent_per_credential;

                if !is_full {
                    let stream_id = self.reserve_stream_locked(
                        &mut concurrency,
                        credential_id,
                        identity,
                        now,
                    );
                    drop(concurrency);
                    self.touch_binding(&api_key, credential_id, now);
                    let mut stats = self.stats.lock();
                    stats.hits = stats.hits.saturating_add(1);
                    return AcquireResult::Acquired {
                        credential_id,
                        stream_id,
                    };
                }

                if per_key_count > 0 && per_key_count < self.max_concurrent_per_key {
                    let stream_id = self.reserve_stream_locked(
                        &mut concurrency,
                        credential_id,
                        identity,
                        now,
                    );
                    drop(concurrency);
                    self.touch_binding(&api_key, credential_id, now);
                    let mut stats = self.stats.lock();
                    stats.queue_jumps = stats.queue_jumps.saturating_add(1);
                    return AcquireResult::Acquired {
                        credential_id,
                        stream_id,
                    };
                }

                if per_key_count == 0 {
                    drop(concurrency);
                    if self.remove_binding_if_matches(&api_key, credential_id) {
                        let mut stats = self.stats.lock();
                        stats.unbinds = stats.unbinds.saturating_add(1);
                    }
                    concurrency = self.concurrency.lock();
                }
            }
        }

        if let Some(credential_id) =
            Self::select_best_credential_locked(&concurrency, available_credentials, self.max_concurrent_per_credential)
        {
            let stream_id =
                self.reserve_stream_locked(&mut concurrency, credential_id, identity, now);
            drop(concurrency);
            {
                let mut bindings = self.bindings.lock();
                bindings.insert(
                    api_key.clone(),
                    StickyBinding {
                        credential_id,
                        last_request_at: now,
                    },
                );
            }
            let mut stats = self.stats.lock();
            stats.assignments = stats.assignments.saturating_add(1);
            return AcquireResult::Acquired {
                credential_id,
                stream_id,
            };
        }

        drop(concurrency);
        {
            let mut stats = self.stats.lock();
            stats.rejections_429 = stats.rejections_429.saturating_add(1);
        }
        AcquireResult::AllFull {
            retry_after_secs: 5.0 + (fastrand::f64() * 5.0),
        }
    }

    /// 获取指定 API key 的绑定凭据（如果存在且未过期）
    pub fn get_bound_credential(&self, api_key: &str) -> Option<u64> {
        let timeout = Duration::from_secs(u64::from(self.sticky_expiry_minutes).saturating_mul(60));
        let now = Instant::now();
        let bindings = self.bindings.lock();
        bindings.get(api_key).and_then(|binding| {
            if now.duration_since(binding.last_request_at) <= timeout {
                Some(binding.credential_id)
            } else {
                None
            }
        })
    }

    pub fn activate_stream(&self, stream_id: StreamId) {
        // 并发计数已在 reserve_stream 时递增，此处仅标记为已激活
        let mut active_streams = self.active_streams.lock();
        if let Some(stream) = active_streams.get_mut(&stream_id) {
            stream.last_touch_at = Instant::now();
            stream.activated = true;
        }
    }

    pub fn cancel_reservation(&self, stream_id: StreamId) {
        let removed = {
            let mut active_streams = self.active_streams.lock();
            active_streams.remove(&stream_id)
        };
        // 预留时已计入并发，取消时需递减
        if let Some(stream) = removed {
            self.decrement_concurrency(stream.credential_id, &stream.api_key);
        }
    }

    pub fn deactivate_stream(&self, stream_id: StreamId) {
        let removed = {
            let mut active_streams = self.active_streams.lock();
            active_streams.remove(&stream_id)
        };

        // 并发计数始终在 reserve_stream 时递增，因此无论是否 activated 都需递减
        if let Some(stream) = removed {
            self.decrement_concurrency(stream.credential_id, &stream.api_key);
        }
    }

    pub fn touch_stream(&self, stream_id: StreamId) {
        let mut active_streams = self.active_streams.lock();
        if let Some(stream) = active_streams.get_mut(&stream_id) {
            stream.last_touch_at = Instant::now();
        }
    }

    pub fn cleanup_zombies(&self) -> usize {
        let timeout = Duration::from_secs(
            u64::from(self.zombie_stream_timeout_minutes).saturating_mul(60),
        );
        // 未激活的预留使用更短的超时（2 分钟），避免误删正在等待上游响应的请求
        let reservation_timeout = Duration::from_secs(120);
        let zombie_ids: Vec<StreamId> = {
            let active_streams = self.active_streams.lock();
            active_streams
                .iter()
                .filter_map(|(stream_id, stream)| {
                    let effective_timeout = if stream.activated {
                        timeout
                    } else {
                        reservation_timeout
                    };
                    if stream.last_touch_at.elapsed() > effective_timeout {
                        Some(*stream_id)
                    } else {
                        None
                    }
                })
                .collect()
        };

        if zombie_ids.is_empty() {
            return 0;
        }

        let mut removed_streams = Vec::new();
        {
            let mut active_streams = self.active_streams.lock();
            for stream_id in zombie_ids {
                if let Some(stream) = active_streams.remove(&stream_id) {
                    removed_streams.push(stream);
                }
            }
        }

        for stream in &removed_streams {
            // 并发计数始终在 reserve_stream 时递增，清理时无论是否 activated 都需递减
            self.decrement_concurrency(stream.credential_id, &stream.api_key);
        }

        removed_streams.len()
    }

    pub fn cleanup_expired_bindings(&self) -> usize {
        let timeout = Duration::from_secs(u64::from(self.sticky_expiry_minutes).saturating_mul(60));
        let now = Instant::now();

        let expired_keys: Vec<String> = {
            let bindings = self.bindings.lock();
            bindings
                .iter()
                .filter_map(|(api_key, binding)| {
                    if now.duration_since(binding.last_request_at) > timeout {
                        Some(api_key.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        if expired_keys.is_empty() {
            return 0;
        }

        let active_keys: HashSet<String> = {
            let active_streams = self.active_streams.lock();
            active_streams
                .values()
                .map(|stream| stream.api_key.clone())
                .collect()
        };

        let mut removed = 0usize;
        {
            let mut bindings = self.bindings.lock();
            for api_key in expired_keys {
                if active_keys.contains(&api_key) {
                    continue;
                }
                let should_remove = bindings
                    .get(&api_key)
                    .map(|binding| now.duration_since(binding.last_request_at) > timeout)
                    .unwrap_or(false);
                if should_remove {
                    bindings.remove(&api_key);
                    removed += 1;
                }
            }
        }

        if removed > 0 {
            let mut stats = self.stats.lock();
            stats.unbinds = stats.unbinds.saturating_add(removed as u64);
        }

        removed
    }

    pub fn snapshot(&self) -> StickySnapshot {
        // Lock ordering: bindings -> concurrency -> active_streams
        let bindings = self.bindings.lock();
        let concurrency = self.concurrency.lock();
        let active_streams = self.active_streams.lock();
        let stats = self.stats.lock();

        let mut bound_keys_by_credential: HashMap<u64, Vec<String>> = HashMap::new();
        for (api_key, binding) in bindings.iter() {
            bound_keys_by_credential
                .entry(binding.credential_id)
                .or_default()
                .push(api_key.clone());
        }

        let mut credential_ids: HashSet<u64> = concurrency.keys().copied().collect();
        credential_ids.extend(bound_keys_by_credential.keys().copied());

        let mut credentials = Vec::with_capacity(credential_ids.len());
        for credential_id in credential_ids {
            let active_count = concurrency
                .get(&credential_id)
                .map(|entry| entry.active_count)
                .unwrap_or(0);
            let mut bound_keys = bound_keys_by_credential
                .remove(&credential_id)
                .unwrap_or_default();
            bound_keys.sort();
            credentials.push(CredentialSnapshot {
                id: credential_id,
                active_count,
                bound_keys,
            });
        }
        credentials.sort_by_key(|snapshot| snapshot.id);

        StickySnapshot {
            credentials,
            stats: stats.clone(),
            active_stream_count: active_streams.len(),
        }
    }

    pub fn stream_snapshots(&self) -> Vec<StreamSnapshot> {
        let active_streams = self.active_streams.lock();
        let mut snapshots: Vec<StreamSnapshot> = active_streams
            .iter()
            .map(|(stream_id, stream)| StreamSnapshot {
                stream_id: *stream_id,
                credential_id: stream.credential_id,
                api_key: stream.api_key.clone(),
                session_id: stream.session_id.clone(),
                activated: stream.activated,
                started_at_secs_ago: stream.last_touch_at.elapsed().as_secs(),
            })
            .collect();
        snapshots.sort_by_key(|stream| stream.stream_id);
        snapshots
    }

    fn current_counts_locked(
        concurrency: &HashMap<u64, CredentialConcurrency>,
        credential_id: u64,
        api_key: &str,
    ) -> (u32, u32) {
        if let Some(entry) = concurrency.get(&credential_id) {
            let per_key_count = entry.per_key_counts.get(api_key).copied().unwrap_or(0);
            (entry.active_count, per_key_count)
        } else {
            (0, 0)
        }
    }

    fn select_best_credential_locked(
        concurrency: &HashMap<u64, CredentialConcurrency>,
        available_credentials: &[AvailableCredential],
        max_concurrent_per_credential: u32,
    ) -> Option<u64> {
        let mut preferred: Option<(u64, u32, usize)> = None;
        let mut fallback: Option<(u64, u32, usize)> = None;

        for (idx, credential) in available_credentials.iter().enumerate() {
            let active_count = concurrency
                .get(&credential.id)
                .map(|entry| entry.active_count)
                .unwrap_or(0);

            if active_count >= max_concurrent_per_credential {
                continue;
            }

            let has_over_half_remaining =
                active_count.saturating_mul(2) < max_concurrent_per_credential;

            let bucket = if has_over_half_remaining {
                &mut preferred
            } else {
                &mut fallback
            };

            match bucket {
                Some((_, best_active_count, best_idx)) => {
                    if active_count < *best_active_count
                        || (active_count == *best_active_count && idx < *best_idx)
                    {
                        *bucket = Some((credential.id, active_count, idx));
                    }
                }
                None => {
                    *bucket = Some((credential.id, active_count, idx));
                }
            }
        }

        preferred.or(fallback).map(|(credential_id, _, _)| credential_id)
    }

    /// 在已持有 concurrency 锁的情况下预留流（原子 check+reserve）
    fn reserve_stream_locked(
        &self,
        concurrency: &mut HashMap<u64, CredentialConcurrency>,
        credential_id: u64,
        identity: &RequestIdentity,
        now: Instant,
    ) -> StreamId {
        let stream_id = self.next_stream_id.fetch_add(1, Ordering::Relaxed);
        let stream = ActiveStream {
            credential_id,
            api_key: identity.api_key.clone(),
            activated: false,
            last_touch_at: now,
            session_id: identity.session_id.clone(),
        };
        let mut active_streams = self.active_streams.lock();
        active_streams.insert(stream_id, stream);
        drop(active_streams);
        // 在同一个 concurrency 锁域内递增
        Self::increment_concurrency_locked(concurrency, credential_id, &identity.api_key);
        stream_id
    }

    fn increment_concurrency_locked(
        concurrency: &mut HashMap<u64, CredentialConcurrency>,
        credential_id: u64,
        api_key: &str,
    ) {
        let entry = concurrency
            .entry(credential_id)
            .or_insert_with(CredentialConcurrency::default);
        entry.active_count = entry.active_count.saturating_add(1);
        let key_count = entry.per_key_counts.entry(api_key.to_string()).or_insert(0);
        *key_count = key_count.saturating_add(1);
    }

    fn decrement_concurrency(&self, credential_id: u64, api_key: &str) {
        let mut concurrency = self.concurrency.lock();
        let mut remove_credential = false;

        if let Some(entry) = concurrency.get_mut(&credential_id) {
            if entry.active_count > 0 {
                entry.active_count -= 1;
            }

            let mut remove_key = false;
            if let Some(key_count) = entry.per_key_counts.get_mut(api_key) {
                if *key_count > 1 {
                    *key_count -= 1;
                } else {
                    remove_key = true;
                }
            }
            if remove_key {
                entry.per_key_counts.remove(api_key);
            }

            remove_credential = entry.active_count == 0 && entry.per_key_counts.is_empty();
        }

        if remove_credential {
            concurrency.remove(&credential_id);
        }
    }

    fn touch_binding(&self, api_key: &str, credential_id: u64, now: Instant) {
        let mut bindings = self.bindings.lock();
        if let Some(binding) = bindings.get_mut(api_key) {
            if binding.credential_id == credential_id {
                binding.last_request_at = now;
            }
        }
    }

    fn remove_binding_if_matches(&self, api_key: &str, credential_id: u64) -> bool {
        let mut bindings = self.bindings.lock();
        let should_remove = bindings
            .get(api_key)
            .map(|binding| binding.credential_id == credential_id)
            .unwrap_or(false);
        if should_remove {
            bindings.remove(api_key);
            return true;
        }
        false
    }
}

pub struct StreamGuard {
    tracker: Arc<StickyTracker>,
    stream_id: StreamId,
    activated: bool,
}

impl StreamGuard {
    pub fn new(tracker: Arc<StickyTracker>, stream_id: StreamId) -> Self {
        Self {
            tracker,
            stream_id,
            activated: false,
        }
    }

    pub fn activate(&mut self) {
        if !self.activated {
            self.tracker.activate_stream(self.stream_id);
            self.activated = true;
        }
    }

    pub fn touch(&self) {
        self.tracker.touch_stream(self.stream_id);
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        // 无论是否 activated，都通过 deactivate_stream 清理
        // （reserve_stream 已计入并发，deactivate_stream 会递减）
        self.tracker.deactivate_stream(self.stream_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker(max_per_cred: u32, max_per_key: u32) -> Arc<StickyTracker> {
        Arc::new(StickyTracker::new(max_per_cred, max_per_key, 30, 15))
    }

    fn creds(ids: &[u64]) -> Vec<AvailableCredential> {
        ids.iter()
            .map(|&id| AvailableCredential {
                id,
                supports_opus: true,
            })
            .collect()
    }

    fn identity(key: &str) -> RequestIdentity {
        RequestIdentity {
            api_key: key.to_string(),
            session_id: None,
        }
    }

    #[test]
    fn test_basic_acquire_and_activate() {
        let t = tracker(2, 5);
        let available = creds(&[1, 2]);
        let id = identity("key-a");

        let result = t.try_acquire(&id, &available);
        let (cred_id, stream_id) = match result {
            AcquireResult::Acquired {
                credential_id,
                stream_id,
            } => (credential_id, stream_id),
            _ => panic!("expected Acquired"),
        };

        // Should be assigned to credential 1 (first available, lowest concurrency)
        assert_eq!(cred_id, 1);

        // Concurrency is already 1 after reservation (before activation)
        let snap = t.snapshot();
        let c1 = snap.credentials.iter().find(|c| c.id == 1).unwrap();
        assert_eq!(c1.active_count, 1);

        // Activate — concurrency stays at 1 (already counted at reservation)
        t.activate_stream(stream_id);
        let snap = t.snapshot();
        let c1 = snap.credentials.iter().find(|c| c.id == 1).unwrap();
        assert_eq!(c1.active_count, 1);
    }

    #[test]
    fn test_sticky_binding_reuse() {
        let t = tracker(2, 5);
        let available = creds(&[1, 2]);
        let id = identity("key-a");

        // First acquire binds to a credential
        let r1 = t.try_acquire(&id, &available);
        let cred1 = match r1 {
            AcquireResult::Acquired { credential_id, stream_id } => {
                t.activate_stream(stream_id);
                credential_id
            }
            _ => panic!("expected Acquired"),
        };

        // Second acquire should reuse the same credential (sticky hit)
        let r2 = t.try_acquire(&id, &available);
        let cred2 = match r2 {
            AcquireResult::Acquired { credential_id, .. } => credential_id,
            _ => panic!("expected Acquired"),
        };
        assert_eq!(cred1, cred2);

        let stats = t.snapshot().stats;
        assert!(stats.hits >= 1);
    }

    #[test]
    fn test_all_full_returns_429() {
        let t = tracker(1, 5);
        let available = creds(&[1]);

        // Fill up credential 1
        let r1 = t.try_acquire(&identity("key-a"), &available);
        match &r1 {
            AcquireResult::Acquired { stream_id, .. } => t.activate_stream(*stream_id),
            _ => panic!("expected Acquired"),
        }

        // Different key, same credential, should get 429
        let r2 = t.try_acquire(&identity("key-b"), &available);
        match r2 {
            AcquireResult::AllFull { retry_after_secs } => {
                assert!(retry_after_secs >= 5.0 && retry_after_secs <= 10.0);
            }
            _ => panic!("expected AllFull"),
        }

        let stats = t.snapshot().stats;
        assert_eq!(stats.rejections_429, 1);
    }

    #[test]
    fn test_deactivate_frees_slot() {
        let t = tracker(1, 5);
        let available = creds(&[1]);

        let stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };
        t.activate_stream(stream_id);

        // Full now
        assert!(matches!(
            t.try_acquire(&identity("key-b"), &available),
            AcquireResult::AllFull { .. }
        ));

        // Deactivate frees the slot
        t.deactivate_stream(stream_id);

        assert!(matches!(
            t.try_acquire(&identity("key-b"), &available),
            AcquireResult::Acquired { .. }
        ));
    }

    #[test]
    fn test_stream_guard_drop_deactivates() {
        let t = tracker(1, 5);
        let available = creds(&[1]);

        let stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };

        {
            let mut guard = StreamGuard::new(t.clone(), stream_id);
            guard.activate();
            // guard dropped here
        }

        // Slot should be free
        assert!(matches!(
            t.try_acquire(&identity("key-b"), &available),
            AcquireResult::Acquired { .. }
        ));
    }

    #[test]
    fn test_stream_guard_drop_cancels_reservation() {
        let t = tracker(2, 5);
        let available = creds(&[1]);

        let stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };

        // Drop without activating — should cancel reservation
        {
            let _guard = StreamGuard::new(t.clone(), stream_id);
        }

        // Stream should be removed
        assert_eq!(t.snapshot().active_stream_count, 0);
    }

    #[test]
    fn test_queue_jump_same_key() {
        let t = tracker(2, 5);
        let available = creds(&[1]);

        // Fill credential to max
        let s1 = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };
        t.activate_stream(s1);

        let s2 = match t.try_acquire(&identity("key-b"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };
        t.activate_stream(s2);

        // Credential is full (2/2). key-a already has active stream, so it can queue-jump
        let r = t.try_acquire(&identity("key-a"), &available);
        match r {
            AcquireResult::Acquired { .. } => {
                let stats = t.snapshot().stats;
                assert!(stats.queue_jumps >= 1);
            }
            _ => panic!("expected queue jump Acquired"),
        }
    }

    #[test]
    fn test_zombie_cleanup() {
        // Use 0-minute timeout so everything is immediately a zombie
        let t = Arc::new(StickyTracker::new(10, 10, 30, 0));
        let available = creds(&[1]);

        let stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };
        t.activate_stream(stream_id);

        // With 0-minute timeout, stream is immediately a zombie
        std::thread::sleep(std::time::Duration::from_millis(10));
        let removed = t.cleanup_zombies();
        assert_eq!(removed, 1);
        assert_eq!(t.snapshot().active_stream_count, 0);
    }

    #[test]
    fn test_expired_binding_cleanup() {
        // Use 0-minute expiry
        let t = Arc::new(StickyTracker::new(10, 10, 0, 15));
        let available = creds(&[1]);

        // Acquire and immediately deactivate
        let stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };
        t.activate_stream(stream_id);
        t.deactivate_stream(stream_id);

        // Binding exists but expired (0-minute TTL)
        std::thread::sleep(std::time::Duration::from_millis(10));
        let removed = t.cleanup_expired_bindings();
        assert_eq!(removed, 1);
    }

    #[test]
    fn test_snapshot_and_stream_snapshots() {
        let t = tracker(5, 5);
        let available = creds(&[1, 2]);

        let s1 = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };
        t.activate_stream(s1);

        let snap = t.snapshot();
        assert_eq!(snap.active_stream_count, 1);
        assert!(!snap.credentials.is_empty());

        let streams = t.stream_snapshots();
        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].api_key, "key-a");
        assert!(streams[0].activated);
    }

    #[test]
    fn test_reservation_counts_toward_concurrency() {
        // max_per_credential = 1, so a single reservation should block further acquires
        let t = tracker(1, 5);
        let available = creds(&[1]);

        // First acquire creates a reservation (not yet activated)
        let _stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };

        // Second acquire from different key should be blocked (reservation counts)
        let r2 = t.try_acquire(&identity("key-b"), &available);
        assert!(
            matches!(r2, AcquireResult::AllFull { .. }),
            "reservation should count toward concurrency limit"
        );
    }

    #[test]
    fn test_cancel_reservation_frees_slot() {
        let t = tracker(1, 5);
        let available = creds(&[1]);

        let stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };

        // Slot is full
        assert!(matches!(
            t.try_acquire(&identity("key-b"), &available),
            AcquireResult::AllFull { .. }
        ));

        // Cancel reservation frees the slot
        t.cancel_reservation(stream_id);

        assert!(matches!(
            t.try_acquire(&identity("key-b"), &available),
            AcquireResult::Acquired { .. }
        ));
    }

    #[test]
    fn test_zombie_cleanup_respects_reservation_timeout() {
        // zombie_stream_timeout = 0 minutes, but reservation timeout is 2 minutes
        let t = Arc::new(StickyTracker::new(10, 10, 30, 0));
        let available = creds(&[1]);

        // Create a reservation (not activated)
        let _stream_id = match t.try_acquire(&identity("key-a"), &available) {
            AcquireResult::Acquired { stream_id, .. } => stream_id,
            _ => panic!("expected Acquired"),
        };

        // With 0-minute zombie timeout but 2-minute reservation timeout,
        // unactivated reservation should NOT be cleaned up immediately
        std::thread::sleep(std::time::Duration::from_millis(10));
        let removed = t.cleanup_zombies();
        assert_eq!(removed, 0, "unactivated reservation should use longer timeout");
        assert_eq!(t.snapshot().active_stream_count, 1);
    }
}

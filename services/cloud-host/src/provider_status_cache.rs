//! In-memory provider status cache to avoid launching Codex on every UI poll.

use std::sync::Arc;
use std::time::{Duration, Instant};

use codex_provider::CodexSubscriptionAvailability;
use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, Ordering};

const CACHE_FRESH: Duration = Duration::from_secs(60);

struct OwnerCacheEntry {
    availability: CodexSubscriptionAvailability,
    cached_at: Instant,
}

struct OwnerRefreshState {
    in_flight: AtomicBool,
}

/// Per-owner cached Codex subscription probe results with deduped background refresh.
#[derive(Clone, Default)]
pub struct ProviderStatusCache {
    entries: Arc<DashMap<String, OwnerCacheEntry>>,
    refresh: Arc<DashMap<String, Arc<OwnerRefreshState>>>,
}

impl ProviderStatusCache {
    pub fn get(&self, owner_id: &str) -> Option<CodexSubscriptionAvailability> {
        self.entries.get(owner_id).map(|entry| {
            let entry = entry.value();
            entry.availability.clone()
        })
    }

    pub fn is_fresh(&self, owner_id: &str) -> bool {
        self.entries
            .get(owner_id)
            .is_some_and(|entry| entry.cached_at.elapsed() < CACHE_FRESH)
    }

    pub fn store(&self, owner_id: &str, availability: CodexSubscriptionAvailability) {
        self.entries.insert(
            owner_id.to_string(),
            OwnerCacheEntry {
                availability,
                cached_at: Instant::now(),
            },
        );
    }

    pub fn invalidate(&self, owner_id: &str) {
        self.entries.remove(owner_id);
    }

    /// Returns true if this caller should run a background refresh (at most one per owner).
    pub fn try_begin_background_refresh(&self, owner_id: &str) -> bool {
        let state = self
            .refresh
            .entry(owner_id.to_string())
            .or_insert_with(|| {
                Arc::new(OwnerRefreshState {
                    in_flight: AtomicBool::new(false),
                })
            })
            .clone();
        state
            .in_flight
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn end_background_refresh(&self, owner_id: &str) {
        if let Some(state) = self.refresh.get(owner_id) {
            state.in_flight.store(false, Ordering::SeqCst);
        }
    }
}

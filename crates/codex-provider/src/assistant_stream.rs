//! Coalesce Codex `item/agentMessage/delta` notifications before durable persistence.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::assistant_accumulator::MessagePhase;

const FLUSH_INTERVAL: Duration = Duration::from_millis(100);
const FLUSH_BYTES: usize = 384;

#[derive(Debug, Clone)]
pub struct CoalescedAssistantDelta {
    pub item_id: String,
    pub phase: MessagePhase,
    pub delta: String,
    pub cumulative_length: usize,
}

#[derive(Debug)]
struct ItemBuffer {
    phase: MessagePhase,
    pending: String,
    cumulative_length: usize,
}

#[derive(Debug)]
pub struct AssistantDeltaCoalescer {
    items: HashMap<String, ItemBuffer>,
    last_flush_at: Instant,
}

impl Default for AssistantDeltaCoalescer {
    fn default() -> Self {
        Self::new()
    }
}

impl AssistantDeltaCoalescer {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            last_flush_at: Instant::now(),
        }
    }

    pub fn ingest(&mut self, item_id: &str, phase: MessagePhase, delta: &str) {
        if delta.is_empty() {
            return;
        }
        let entry = self
            .items
            .entry(item_id.to_string())
            .or_insert_with(|| ItemBuffer {
                phase,
                pending: String::new(),
                cumulative_length: 0,
            });
        entry.phase = phase;
        entry.pending.push_str(delta);
        entry.cumulative_length += delta.len();
    }

    pub fn pending_bytes(&self) -> usize {
        self.items.values().map(|item| item.pending.len()).sum()
    }

    pub fn should_flush(&self, now: Instant) -> bool {
        if self.pending_bytes() == 0 {
            return false;
        }
        now.duration_since(self.last_flush_at) >= FLUSH_INTERVAL
            || self.pending_bytes() >= FLUSH_BYTES
    }

    pub fn take_if_due(&mut self, now: Instant) -> Vec<CoalescedAssistantDelta> {
        if !self.should_flush(now) {
            return Vec::new();
        }
        self.flush_all()
    }

    pub fn flush_all(&mut self) -> Vec<CoalescedAssistantDelta> {
        if self.items.is_empty() {
            return Vec::new();
        }
        self.last_flush_at = Instant::now();
        let mut out = Vec::new();
        for (item_id, item) in self.items.drain() {
            if item.pending.is_empty() {
                continue;
            }
            out.push(CoalescedAssistantDelta {
                item_id,
                phase: item.phase,
                delta: item.pending,
                cumulative_length: item.cumulative_length,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn coalesces_until_byte_threshold() {
        let mut coalescer = AssistantDeltaCoalescer::new();
        coalescer.ingest("m1", MessagePhase::FinalAnswer, &"a".repeat(400));
        let flushed = coalescer.take_if_due(Instant::now());
        assert_eq!(flushed.len(), 1);
        assert!(flushed[0].delta.len() >= 400);
    }
}

pub fn phase_to_event_str(phase: MessagePhase) -> &'static str {
    match phase {
        MessagePhase::Commentary => "commentary",
        MessagePhase::FinalAnswer => "final_answer",
        MessagePhase::Unknown => "unknown",
    }
}

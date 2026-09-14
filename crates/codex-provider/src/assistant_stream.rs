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
    /// Inclusive start byte offset for this chunk within the item stream.
    pub start_offset: usize,
    /// Exclusive end byte offset (monotonic for the lifetime of the item).
    pub end_offset: usize,
}

#[derive(Debug)]
struct ItemBuffer {
    phase: MessagePhase,
    pending: String,
    flushed_end_offset: usize,
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
                flushed_end_offset: 0,
            });
        entry.phase = phase;
        entry.pending.push_str(delta);
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
        for (item_id, item) in self.items.iter_mut() {
            if item.pending.is_empty() {
                continue;
            }
            let start_offset = item.flushed_end_offset;
            let end_offset = start_offset + item.pending.len();
            out.push(CoalescedAssistantDelta {
                item_id: item_id.clone(),
                phase: item.phase,
                delta: item.pending.clone(),
                start_offset,
                end_offset,
            });
            item.flushed_end_offset = end_offset;
            item.pending.clear();
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

    #[test]
    fn multiple_flushes_preserve_monotonic_offsets_and_replay() {
        let mut coalescer = AssistantDeltaCoalescer::new();
        let item = "m1";
        coalescer.ingest(item, MessagePhase::FinalAnswer, "hel");
        let first = coalescer.flush_all();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].start_offset, 0);
        assert_eq!(first[0].end_offset, 3);
        assert_eq!(first[0].delta, "hel");

        coalescer.ingest(item, MessagePhase::FinalAnswer, "lo ");
        let second = coalescer.flush_all();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].start_offset, 3);
        assert_eq!(second[0].end_offset, 6);
        assert_eq!(second[0].delta, "lo ");

        coalescer.ingest(item, MessagePhase::FinalAnswer, "world");
        let third = coalescer.flush_all();
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].start_offset, 6);
        assert_eq!(third[0].end_offset, 11);
        assert_eq!(third[0].delta, "world");

        let chunks = [first, second, third].concat();
        let replay_text = |events: &[CoalescedAssistantDelta]| {
            let mut progress = std::collections::HashMap::new();
            let mut text = String::new();
            for chunk in events {
                let previous = progress.get(&chunk.item_id).copied().unwrap_or(0);
                if chunk.end_offset <= previous {
                    continue;
                }
                if chunk.start_offset < previous {
                    continue;
                }
                text.push_str(&chunk.delta);
                progress.insert(chunk.item_id.clone(), chunk.end_offset);
            }
            text
        };

        let once = replay_text(&chunks);
        assert_eq!(once, "hello world");
        let twice = replay_text(&chunks.iter().chain(chunks.iter()).cloned().collect::<Vec<_>>());
        assert_eq!(twice, "hello world");
    }
}

pub fn phase_to_event_str(phase: MessagePhase) -> &'static str {
    match phase {
        MessagePhase::Commentary => "commentary",
        MessagePhase::FinalAnswer => "final_answer",
        MessagePhase::Unknown => "unknown",
    }
}

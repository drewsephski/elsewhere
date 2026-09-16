//! Per-item Codex `agentMessage` accumulation and canonical final-answer selection.

use std::collections::HashMap;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagePhase {
    Commentary,
    FinalAnswer,
    Unknown,
}

#[derive(Debug, Clone)]
struct AssistantItemState {
    item_id: String,
    phase: MessagePhase,
    delta_buffer: String,
    completed_text: Option<String>,
    /// Monotonic order assigned on first sight of this item (started/delta/completed).
    insertion_order: u64,
    /// Set when `on_agent_message_completed` runs; 0 means not completed yet.
    completion_order: u64,
}

#[derive(Debug, Default)]
pub struct CodexAssistantAccumulator {
    items: HashMap<String, AssistantItemState>,
    next_item_order: u64,
    next_completion_order: u64,
    final_answer: Option<String>,
    latest_unknown_answer: Option<String>,
    latest_any_answer: Option<String>,
    latest_commentary: Option<String>,
}

impl CodexAssistantAccumulator {
    fn alloc_insertion_order(&mut self) -> u64 {
        self.next_item_order += 1;
        self.next_item_order
    }

    fn new_item_state(
        item_id: String,
        phase: MessagePhase,
        insertion_order: u64,
    ) -> AssistantItemState {
        AssistantItemState {
            item_id,
            phase,
            delta_buffer: String::new(),
            completed_text: None,
            insertion_order,
            completion_order: 0,
        }
    }

    pub fn on_item_started(&mut self, item: &Value) {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if item_type != "agentMessage" {
            return;
        }
        let Some(item_id) = item.get("id").and_then(|v| v.as_str()) else {
            return;
        };
        let phase = parse_message_phase(item.get("phase"));
        let key = item_id.to_string();
        if let Some(existing) = self.items.get_mut(&key) {
            existing.phase = phase;
            return;
        }
        let insertion_order = self.alloc_insertion_order();
        self.items.insert(
            key,
            Self::new_item_state(item_id.to_string(), phase, insertion_order),
        );
    }

    pub fn on_agent_message_delta(&mut self, params: &Value) {
        let Some(item_id) = params.get("itemId").and_then(|v| v.as_str()) else {
            return;
        };
        let Some(delta) = params.get("delta").and_then(|v| v.as_str()) else {
            return;
        };
        let phase = self
            .items
            .get(item_id)
            .map(|i| i.phase)
            .unwrap_or(MessagePhase::Unknown);
        let key = item_id.to_string();
        if !self.items.contains_key(&key) {
            let insertion_order = self.alloc_insertion_order();
            self.items.insert(
                key.clone(),
                Self::new_item_state(key.clone(), phase, insertion_order),
            );
        }
        self.items
            .get_mut(&key)
            .expect("item inserted above")
            .delta_buffer
            .push_str(delta);
    }

    pub fn on_agent_message_completed(&mut self, item: &Value) {
        let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if item_type != "agentMessage" {
            return;
        }
        let item_id = item
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let text = item
            .get("text")
            .or_else(|| item.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let phase = parse_message_phase(item.get("phase"));

        self.next_completion_order += 1;
        let order = self.next_completion_order;

        if !self.items.contains_key(&item_id) {
            let insertion_order = self.alloc_insertion_order();
            self.items.insert(
                item_id.clone(),
                Self::new_item_state(item_id.clone(), phase, insertion_order),
            );
        }
        let entry = self.items.get_mut(&item_id).expect("item inserted above");
        entry.phase = phase;
        entry.completed_text = Some(text.clone());
        entry.completion_order = order;

        if text.is_empty() {
            return;
        }

        self.latest_any_answer = Some(text.clone());

        match phase {
            MessagePhase::FinalAnswer => {
                self.final_answer = Some(text);
            }
            MessagePhase::Commentary => {
                self.latest_commentary = Some(text);
            }
            MessagePhase::Unknown => {
                self.latest_unknown_answer = Some(text);
            }
        }
    }

    /// Successful turn: prefer `turn.items` snapshot, then notification-tracked state.
    pub fn canonical_success(&self, turn_completed: Option<&Value>) -> String {
        if let Some(params) = turn_completed {
            if let Some(from_turn) = assistant_from_turn_items(params) {
                return from_turn;
            }
        }
        self.canonical_from_notifications()
    }

    /// Live assistant bubble text: final_answer and unknown phases only (never commentary).
    pub fn streaming_answer_text(&self) -> String {
        if let Some(text) = self.final_answer_from_buffers() {
            return text;
        }
        if let Some(text) = self.unknown_from_buffers() {
            return text;
        }
        String::new()
    }

    fn final_answer_from_buffers(&self) -> Option<String> {
        self.latest_delta_for_phase(MessagePhase::FinalAnswer, true)
    }

    fn unknown_from_buffers(&self) -> Option<String> {
        self.latest_delta_for_phase(MessagePhase::Unknown, true)
    }

    fn latest_delta_for_phase(
        &self,
        phase: MessagePhase,
        include_in_progress: bool,
    ) -> Option<String> {
        let mut ordered: Vec<&AssistantItemState> = self.items.values().collect();
        ordered.sort_by(|a, b| {
            a.completion_order
                .cmp(&b.completion_order)
                .then_with(|| a.insertion_order.cmp(&b.insertion_order))
                .then_with(|| a.item_id.cmp(&b.item_id))
        });
        for item in ordered.iter().rev() {
            if item.phase != phase {
                continue;
            }
            if let Some(ref completed) = item.completed_text {
                if !completed.is_empty() {
                    return Some(completed.clone());
                }
            }
            if include_in_progress && !item.delta_buffer.is_empty() {
                return Some(item.delta_buffer.clone());
            }
        }
        None
    }

    pub fn phase_for_item(&self, item_id: &str) -> MessagePhase {
        self.items
            .get(item_id)
            .map(|item| item.phase)
            .unwrap_or(MessagePhase::Unknown)
    }

    /// Best-effort text when the turn did not complete successfully.
    pub fn partial_output(&self) -> String {
        if let Some(text) = self.final_answer.clone() {
            return text;
        }
        if let Some(text) = self.latest_unknown_answer.clone() {
            return text;
        }
        if let Some(text) = self.latest_delta_buffer(false) {
            return text;
        }
        if let Some(text) = self.latest_commentary.clone() {
            return text;
        }
        if let Some(text) = self.latest_any_answer.clone() {
            return text;
        }
        self.latest_delta_buffer(true).unwrap_or_default()
    }

    fn canonical_from_notifications(&self) -> String {
        if let Some(text) = self.final_answer.clone() {
            return text;
        }
        if let Some(text) = self.latest_unknown_answer.clone() {
            return text;
        }
        if let Some(text) = self.latest_any_answer.clone() {
            return text;
        }
        if let Some(text) = self.latest_delta_buffer(false) {
            return text;
        }
        self.latest_delta_buffer(true).unwrap_or_default()
    }

    fn latest_delta_buffer(&self, include_commentary: bool) -> Option<String> {
        let mut ordered: Vec<&AssistantItemState> = self.items.values().collect();
        ordered.sort_by(|a, b| {
            a.completion_order
                .cmp(&b.completion_order)
                .then_with(|| a.insertion_order.cmp(&b.insertion_order))
                .then_with(|| a.item_id.cmp(&b.item_id))
        });
        for item in ordered.iter().rev() {
            if !include_commentary && item.phase == MessagePhase::Commentary {
                continue;
            }
            if let Some(ref completed) = item.completed_text {
                if !completed.is_empty() {
                    return Some(completed.clone());
                }
            }
            if !item.delta_buffer.is_empty() {
                return Some(item.delta_buffer.clone());
            }
        }
        None
    }
}

fn parse_message_phase(value: Option<&Value>) -> MessagePhase {
    match value.and_then(|v| v.as_str()) {
        Some("commentary") => MessagePhase::Commentary,
        Some("final_answer") => MessagePhase::FinalAnswer,
        _ => MessagePhase::Unknown,
    }
}

fn assistant_text_from_item(item: &Value) -> String {
    item.get("text")
        .or_else(|| item.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Authoritative selection from `turn/completed` → `turn.items`.
pub fn assistant_from_turn_items(turn_completed: &Value) -> Option<String> {
    let items = turn_completed
        .get("turn")
        .and_then(|t| t.get("items"))
        .and_then(|v| v.as_array())?;

    let agent_messages: Vec<&Value> = items
        .iter()
        .filter(|item| item.get("type").and_then(|v| v.as_str()) == Some("agentMessage"))
        .collect();
    if agent_messages.is_empty() {
        return None;
    }

    for item in agent_messages.iter().rev() {
        let phase = parse_message_phase(item.get("phase"));
        if phase == MessagePhase::FinalAnswer {
            let text = assistant_text_from_item(item);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    for item in agent_messages.iter().rev() {
        if parse_message_phase(item.get("phase")) == MessagePhase::Unknown {
            let text = assistant_text_from_item(item);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    for item in agent_messages.iter().rev() {
        let text = assistant_text_from_item(item);
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn apply_sequence(acc: &mut CodexAssistantAccumulator, events: &[Value]) {
        for event in events {
            let method = event.get("method").and_then(|v| v.as_str()).unwrap();
            let params = event.get("params").unwrap();
            match method {
                "item/started" => {
                    if let Some(item) = params.get("item") {
                        acc.on_item_started(item);
                    }
                }
                "item/agentMessage/delta" => acc.on_agent_message_delta(params),
                "item/completed" => {
                    if let Some(item) = params.get("item") {
                        acc.on_agent_message_completed(item);
                    }
                }
                "turn/completed" => {}
                _ => {}
            }
        }
    }

    #[test]
    fn commentary_then_final_answer_uses_final_only() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/started",
                    "params": { "item": { "type": "agentMessage", "id": "commentary-1", "phase": "commentary" } }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "commentary-1", "delta": "I'll check that." }
                }),
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "commentary-1", "phase": "commentary", "text": "I'll check that." } }
                }),
                json!({
                    "method": "item/started",
                    "params": { "item": { "type": "agentMessage", "id": "final-1", "phase": "final_answer" } }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "final-1", "delta": "hello from Elsewhere via ChatGPT subscription" }
                }),
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "final-1", "phase": "final_answer", "text": "hello from Elsewhere via ChatGPT subscription" } }
                }),
            ],
        );

        let result = acc.canonical_success(None);
        assert_eq!(result, "hello from Elsewhere via ChatGPT subscription");
        assert!(!result.contains("I'll check that."));
    }

    #[test]
    fn unknown_phase_latest_completed_wins() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "a", "text": "first" } }
                }),
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "b", "text": "second" } }
                }),
            ],
        );
        assert_eq!(acc.canonical_success(None), "second");
    }

    #[test]
    fn final_answer_beats_unknown_phase() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "u", "text": "unknown body" } }
                }),
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "f", "phase": "final_answer", "text": "final body" } }
                }),
            ],
        );
        assert_eq!(acc.canonical_success(None), "final body");
    }

    #[test]
    fn multiple_final_answers_latest_wins() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "f1", "phase": "final_answer", "text": "first final" } }
                }),
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "f2", "phase": "final_answer", "text": "second final" } }
                }),
            ],
        );
        assert_eq!(acc.canonical_success(None), "second final");
    }

    #[test]
    fn delta_fallback_when_no_completion() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/started",
                    "params": { "item": { "type": "agentMessage", "id": "m1" } }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "m1", "delta": "partial output" }
                }),
            ],
        );
        assert_eq!(acc.canonical_success(None), "partial output");
        assert_eq!(acc.partial_output(), "partial output");
    }

    #[test]
    fn single_completed_agent_message_unchanged() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "msg-1", "delta": "hello " }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "msg-1", "delta": "world" }
                }),
                json!({
                    "method": "item/completed",
                    "params": { "item": { "type": "agentMessage", "id": "msg-1", "text": "hello world" } }
                }),
            ],
        );
        assert_eq!(acc.canonical_success(None), "hello world");
    }

    #[test]
    fn turn_items_snapshot_overrides_notifications() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[json!({
                "method": "item/completed",
                "params": { "item": { "type": "agentMessage", "id": "c", "phase": "commentary", "text": "noise" } }
            })],
        );
        let turn = json!({
            "turn": {
                "items": [
                    { "type": "agentMessage", "id": "f", "phase": "final_answer", "text": "from snapshot" }
                ]
            }
        });
        assert_eq!(acc.canonical_success(Some(&turn)), "from snapshot");
    }

    #[test]
    fn latest_incomplete_delta_prefers_latest_item_deterministically() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/started",
                    "params": { "item": { "type": "agentMessage", "id": "older" } }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "older", "delta": "first item" }
                }),
                json!({
                    "method": "item/started",
                    "params": { "item": { "type": "agentMessage", "id": "newer" } }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "newer", "delta": "second item" }
                }),
            ],
        );
        for _ in 0..50 {
            assert_eq!(acc.partial_output(), "second item");
            assert_eq!(acc.canonical_from_notifications(), "second item");
        }
    }

    #[test]
    fn deltas_do_not_concatenate_across_items_for_canonical() {
        let mut acc = CodexAssistantAccumulator::default();
        apply_sequence(
            &mut acc,
            &[
                json!({
                    "method": "item/started",
                    "params": { "item": { "type": "agentMessage", "id": "c", "phase": "commentary" } }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "c", "delta": "commentary delta" }
                }),
                json!({
                    "method": "item/started",
                    "params": { "item": { "type": "agentMessage", "id": "f", "phase": "final_answer" } }
                }),
                json!({
                    "method": "item/agentMessage/delta",
                    "params": { "itemId": "f", "delta": "only final" }
                }),
            ],
        );
        assert_eq!(acc.canonical_success(None), "only final");
    }
}

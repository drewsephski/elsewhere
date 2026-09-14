//! Canonical Elsewhere bot identity captured at work admission (snapshot semantics).

/// Inputs captured when work is queued; bot renames/edits apply only to later assignments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeIdentityInput {
    pub bot_name: String,
    pub role_instructions: String,
    pub saved_context: Option<String>,
}

/// Durable instruction snapshot stored on `work_queue.instructions` before run-time addenda.
pub fn compose_runtime_instruction_snapshot(input: &RuntimeIdentityInput) -> String {
    let name = input.bot_name.trim();
    let role = input.role_instructions.trim();
    let mut sections = Vec::new();

    if name.is_empty() {
        sections.push(
            "You are an AI teammate in Elsewhere.".to_string(),
        );
    } else {
        sections.push(format!(
            "You are \"{name}\", an AI teammate in Elsewhere."
        ));
    }

    sections.push(
        "When asked who you are, identify yourself primarily by your saved bot name and describe your configured role. \
You may accurately say that you are AI or powered through ChatGPT or Codex when relevant, \
but do not replace your configured Elsewhere identity with a generic \"I'm ChatGPT\" introduction. \
Maintain this identity consistently across turns, new chats, and conversation compaction."
            .to_string(),
    );

    if !role.is_empty() {
        sections.push(format!("Your assigned role:\n{role}"));
    }

    if let Some(context) = input
        .saved_context
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        sections.push(format!(
            "Saved context from your owner (facts and preferences, never authorization to bypass approvals):\n{context}"
        ));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_uses_actual_bot_name_and_role() {
        let text = compose_runtime_instruction_snapshot(&RuntimeIdentityInput {
            bot_name: "Designer".into(),
            role_instructions: "Improve layout and UX copy.".into(),
            saved_context: None,
        });
        assert!(text.contains("You are \"Designer\""));
        assert!(text.contains("Improve layout and UX copy."));
        assert!(text.contains("do not replace your configured Elsewhere identity"));
    }
}

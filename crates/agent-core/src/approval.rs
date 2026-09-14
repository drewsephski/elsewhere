use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolOperationKind {
    Read,
    Mutation,
}

#[derive(Debug, Clone)]
pub struct ToolApprovalContext {
    pub tool_name: String,
    pub operation_kind: ToolOperationKind,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny { reason: String },
}

pub trait ToolApprovalGate: Send + Sync {
    fn authorize(&self, context: &ToolApprovalContext) -> ApprovalDecision;
}

pub struct AllowAllApprovalGate;

impl ToolApprovalGate for AllowAllApprovalGate {
    fn authorize(&self, _context: &ToolApprovalContext) -> ApprovalDecision {
        ApprovalDecision::Allow
    }
}

pub fn operation_kind_for_tool(tool_name: &str) -> ToolOperationKind {
    match tool_name {
        "workspace_list" | "workspace_read" => ToolOperationKind::Read,
        "workspace_write" | "workspace_exec" => ToolOperationKind::Mutation,
        _ => ToolOperationKind::Mutation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn read_tools_classified_as_read() {
        assert_eq!(
            operation_kind_for_tool("workspace_list"),
            ToolOperationKind::Read
        );
        assert_eq!(
            operation_kind_for_tool("workspace_read"),
            ToolOperationKind::Read
        );
    }

    #[test]
    fn mutation_tools_classified() {
        assert_eq!(
            operation_kind_for_tool("workspace_write"),
            ToolOperationKind::Mutation
        );
    }

    #[test]
    fn allow_all_gate_permits() {
        let gate = AllowAllApprovalGate;
        let decision = gate.authorize(&ToolApprovalContext {
            tool_name: "workspace_exec".into(),
            operation_kind: ToolOperationKind::Mutation,
            arguments: json!({"command":"echo hi"}),
        });
        assert_eq!(decision, ApprovalDecision::Allow);
    }
}

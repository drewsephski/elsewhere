use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::attachment_tools::dispatch_attachment_tool;
use crate::collaboration::{AgentCollaboration, CollaborationContext, CollaborationError};
use crate::connector_tools::dispatch_connector_tool_with_gate;
use crate::github_coding::AgentGithubCoding;
use crate::github_coding_tools::dispatch_github_coding_tool;
use crate::human_intervention::is_human_intervention_tool;
use crate::human_intervention_tools::dispatch_human_intervention_tool;
use crate::memory::MemoryContext;
use crate::memory_tools::dispatch_memory_tool;
use crate::subagent::{
    AgentSubagents, SubagentContext, SubagentError, SubagentRequest, RUN_SUBAGENT_DESCRIPTION,
    RUN_SUBAGENT_TOOL_NAME,
};
use crate::routine_tools::dispatch_routine_tool;
use crate::routines::RoutineContext;
use crate::skill_tools::dispatch_skill_tool;
use crate::skills::SkillContext;
use crate::tool_catalog::{
    is_attachment_tool, is_collaboration_tool, is_connector_tool, is_github_coding_tool,
    is_memory_tool, is_routine_tool, is_skill_tool, is_subagent_tool, is_user_question_tool,
};
use crate::tools::ToolError;
use crate::user_question_tools::dispatch_user_question_tool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::browser_recovery::BrowserRecoverySession;

pub fn collaboration_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "bot_list",
            "description": "List other Bots owned by the same user that you may hand work to asynchronously. Does not wait for them to finish.",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "bot_delegate",
            "description": "Hand a piece of work to another Bot asynchronously. This only queues durable work for the recipient; it does NOT run their task or wait for completion. Tell the user what you delegated and to whom; do not claim the other Bot already finished.",
            "parameters": {
                "type": "object",
                "properties": {
                    "targetBotId": { "type": "string", "description": "Recipient Bot id from bot_list" },
                    "instruction": { "type": "string", "description": "Clear task for the recipient to own" },
                    "context": { "type": "string", "description": "Optional bounded text context (not filesystem paths from your computer)" },
                    "onComplete": {
                        "type": "string",
                        "enum": ["resume_source", "none"],
                        "description": "resume_source: queue a follow-up on the source Bot when the recipient finishes; none: hand off without source follow-up"
                    }
                },
                "required": ["targetBotId", "instruction"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": RUN_SUBAGENT_TOOL_NAME,
            "description": RUN_SUBAGENT_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Short human-readable helper name, such as Research, Reviewer, or Planner"
                    },
                    "task": {
                        "type": "string",
                        "description": "The focused assignment for this helper"
                    },
                    "context": {
                        "type": "string",
                        "description": "Optional bounded supporting context. Do not paste the entire transcript."
                    }
                },
                "required": ["name", "task"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub fn all_openai_tool_definitions() -> Vec<Value> {
    let mut tools = crate::tools::openai_tool_definitions();
    tools.extend(crate::human_intervention_tools::human_intervention_openai_tool_definitions());
    tools.extend(crate::user_question_tools::user_question_openai_tool_definitions());
    tools.extend(crate::attachment_tools::attachment_openai_tool_definitions());
    tools.extend(collaboration_openai_tool_definitions());
    tools.extend(crate::connector_tools::connector_openai_tool_definitions());
    tools.extend(crate::memory_tools::memory_openai_tool_definitions());
    tools.extend(crate::routine_tools::routine_openai_tool_definitions());
    tools.extend(crate::skill_tools::skill_openai_tool_definitions());
    tools.extend(crate::github_coding_tools::github_coding_openai_tool_definitions());
    tools
}

pub async fn dispatch_agent_tool_with_gate(
    computer: &dyn crate::computer::AgentComputer,
    collaboration: Option<&Arc<dyn AgentCollaboration>>,
    connectors: Option<&Arc<dyn crate::connectors::AgentConnectors>>,
    human_intervention: Option<&Arc<dyn crate::human_intervention::AgentHumanIntervention>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    collaboration_ctx: Option<&CollaborationContext>,
) -> Result<Value, ToolError> {
    dispatch_agent_tool_with_gate_and_recovery(
        computer,
        collaboration,
        connectors,
        human_intervention,
        None, // subagents
        None, // memory
        None, // routines
        None, // skills
        None, // github_coding
        None, // attachments
        None, // user_questions
        name,
        arguments,
        cancel,
        gate,
        run,
        collaboration_ctx,
        None, // browser_recovery
    )
    .await
}

pub async fn dispatch_agent_tool_with_gate_and_recovery(
    computer: &dyn crate::computer::AgentComputer,
    collaboration: Option<&Arc<dyn AgentCollaboration>>,
    connectors: Option<&Arc<dyn crate::connectors::AgentConnectors>>,
    human_intervention: Option<&Arc<dyn crate::human_intervention::AgentHumanIntervention>>,
    subagents: Option<&Arc<dyn AgentSubagents>>,
    memory: Option<&Arc<dyn crate::memory::AgentMemory>>,
    routines: Option<&Arc<dyn crate::routines::AgentRoutines>>,
    skills: Option<&Arc<dyn crate::skills::AgentSkills>>,
    github_coding: Option<&Arc<dyn AgentGithubCoding>>,
    attachments: Option<&Arc<dyn crate::attachments::AgentAttachments>>,
    user_questions: Option<&Arc<dyn crate::user_question::AgentUserQuestion>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    collaboration_ctx: Option<&CollaborationContext>,
    browser_recovery: Option<&Arc<BrowserRecoverySession>>,
) -> Result<Value, ToolError> {
    if is_routine_tool(name) {
        let routine_ctx = collaboration_ctx.map(|ctx| RoutineContext {
            owner_id: ctx.owner_id.clone(),
            bot_id: ctx.source_bot_id.clone(),
            source_conversation_id: ctx.source_conversation_id.clone(),
        });
        return dispatch_routine_tool(
            routines,
            name,
            arguments,
            cancel,
            gate,
            run,
            routine_ctx.as_ref(),
        )
        .await;
    }
    if is_skill_tool(name) {
        let skill_ctx = collaboration_ctx.map(|ctx| SkillContext {
            owner_id: ctx.owner_id.clone(),
            bot_id: ctx.source_bot_id.clone(),
            source_conversation_id: ctx.source_conversation_id.clone(),
        });
        return dispatch_skill_tool(
            skills,
            name,
            arguments,
            cancel,
            gate,
            run,
            skill_ctx.as_ref(),
        )
        .await;
    }
    if is_human_intervention_tool(name) {
        return dispatch_human_intervention_tool(
            human_intervention,
            name,
            arguments,
            cancel,
            gate,
            run,
            browser_recovery,
        )
        .await;
    }
    if is_user_question_tool(name) {
        return dispatch_user_question_tool(user_questions, name, arguments, cancel, gate, run)
            .await;
    }
    if is_attachment_tool(name) {
        return dispatch_attachment_tool(attachments, name, arguments, cancel, gate, run).await;
    }
    if is_memory_tool(name) {
        let memory_ctx = collaboration_ctx.map(|ctx| MemoryContext {
            owner_id: ctx.owner_id.clone(),
            bot_id: ctx.source_bot_id.clone(),
            run_id: ctx.source_run_id.clone(),
            request_id: ctx.source_request_id.clone(),
            source_message_id: None,
            tool_invocation_id: ctx.tool_invocation_id.clone(),
        });
        return dispatch_memory_tool(
            memory,
            name,
            arguments,
            cancel,
            gate,
            run,
            memory_ctx.as_ref(),
        )
        .await;
    }
    if is_subagent_tool(name) {
        return dispatch_subagent_tool(
            subagents,
            name,
            arguments,
            cancel,
            gate,
            run,
            collaboration_ctx,
        )
        .await;
    }
    if is_collaboration_tool(name) {
        return dispatch_collaboration_tool(
            collaboration,
            name,
            arguments,
            cancel,
            gate,
            run,
            collaboration_ctx,
        )
        .await;
    }
    if is_github_coding_tool(name) {
        return dispatch_github_coding_tool(
            github_coding,
            computer,
            name,
            arguments,
            cancel,
            gate,
            run,
        )
        .await;
    }
    if is_connector_tool(name) {
        return dispatch_connector_tool_with_gate(connectors, name, arguments, cancel, gate, run)
            .await;
    }
    crate::tools::dispatch_tool_with_gate_and_recovery(
        computer,
        name,
        arguments,
        cancel,
        gate,
        run,
        browser_recovery,
    )
    .await
}

async fn dispatch_collaboration_tool(
    collaboration: Option<&Arc<dyn AgentCollaboration>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    collaboration_ctx: Option<&CollaborationContext>,
) -> Result<Value, ToolError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }
    let service = collaboration.ok_or_else(|| {
        ToolError::MalformedArguments("bot collaboration is not available in this run".into())
    })?;
    let ctx = collaboration_ctx
        .ok_or_else(|| ToolError::MalformedArguments("collaboration context missing".into()))?;

    let args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;

    let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
    let approval = gate
        .authorize(&approval_ctx)
        .await
        .map_err(map_approval_error)?;
    if let crate::approval::ApprovalDecision::Deny { reason } = approval {
        return Err(ToolError::Denied(reason));
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let result = match name {
        "bot_list" => {
            let bots = service
                .list_bots(ctx)
                .await
                .map_err(map_collaboration_error)?;
            Ok(json!({ "ok": true, "bots": bots }))
        }
        "bot_delegate" => {
            let target_bot_id = required_str(&args, "targetBotId")?;
            let instruction = required_str(&args, "instruction")?;
            let context = args
                .get("context")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty());
            let return_policy = args
                .get("onComplete")
                .and_then(|v| v.as_str())
                .unwrap_or("resume_source");
            if return_policy != "resume_source" && return_policy != "none" {
                return Err(ToolError::MalformedArguments(
                    "onComplete must be resume_source or none".into(),
                ));
            }
            let enqueued = service
                .delegate(ctx, target_bot_id, instruction, context, return_policy)
                .await
                .map_err(map_collaboration_error)?;
            Ok(json!({
                "ok": true,
                "delegationId": enqueued.delegation_id,
                "targetBotId": enqueued.target_bot_id,
                "targetBotName": enqueued.target_bot_name,
                "targetRunId": enqueued.target_run_id,
                "status": enqueued.status,
                "async": true,
                "detail": "Work queued for the recipient Bot. They have not finished yet."
            }))
        }
        other => Err(ToolError::MalformedArguments(format!(
            "unknown tool: {other}"
        ))),
    }?;

    let mut envelope = result;
    if let Some(obj) = envelope.as_object_mut() {
        obj.insert("tool".into(), json!(name));
    }
    Ok(envelope)
}

async fn dispatch_subagent_tool(
    subagents: Option<&Arc<dyn AgentSubagents>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    collaboration_ctx: Option<&CollaborationContext>,
) -> Result<Value, ToolError> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }
    let service = subagents.ok_or_else(|| {
        ToolError::MalformedArguments("subagents are not available in this run".into())
    })?;
    let collab = collaboration_ctx
        .ok_or_else(|| ToolError::MalformedArguments("subagent context missing".into()))?;
    let args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;

    let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
    let approval = gate
        .authorize(&approval_ctx)
        .await
        .map_err(map_approval_error)?;
    if let crate::approval::ApprovalDecision::Deny { reason } = approval {
        return Ok(json!({
            "ok": false,
            "tool": name,
            "status": "denied",
            "error": reason,
            "detail": "The helper was not launched."
        }));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let request = SubagentRequest {
        name: required_str(&args, "name")?.to_string(),
        task: required_str(&args, "task")?.to_string(),
        context: args
            .get("context")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    };
    let ctx = SubagentContext {
        owner_id: collab.owner_id.clone(),
        bot_id: collab.source_bot_id.clone(),
        parent_run_id: collab.source_run_id.clone(),
        parent_request_id: collab.source_request_id.clone(),
        tool_invocation_id: collab.tool_invocation_id.clone(),
        model: String::new(),
        cancel: Arc::new(AtomicBool::new(cancel.load(Ordering::Relaxed))),
    };
    let outcome = service
        .run_subagent(&ctx, request)
        .await
        .map_err(map_subagent_error)?;
    Ok(json!({
        "ok": outcome.status == "completed",
        "tool": name,
        "subagentId": outcome.subagent_id,
        "name": outcome.name,
        "status": outcome.status,
        "result": outcome.result,
        "error": outcome.error,
        "detail": "This was a temporary helper for the current assignment, not another Bot."
    }))
}

fn map_subagent_error(err: SubagentError) -> ToolError {
    match err {
        SubagentError::Cancelled => ToolError::Cancelled,
        SubagentError::Validation(m) | SubagentError::Unsupported(m) => {
            ToolError::MalformedArguments(m)
        }
        SubagentError::LimitExceeded(m) | SubagentError::Internal(m) => ToolError::Denied(m),
    }
}

fn map_collaboration_error(err: CollaborationError) -> ToolError {
    match err {
        CollaborationError::NotFound => ToolError::Denied("Bot not found".into()),
        CollaborationError::Validation(m) => ToolError::MalformedArguments(m),
        CollaborationError::LimitExceeded(m) => ToolError::Denied(m),
        CollaborationError::Conflict(m) => ToolError::MalformedArguments(m),
        CollaborationError::Internal(m) => ToolError::MalformedArguments(m),
    }
}

fn map_approval_error(err: crate::approval::ApprovalError) -> ToolError {
    match err {
        crate::approval::ApprovalError::Cancelled => ToolError::Cancelled,
        crate::approval::ApprovalError::Denied { reason } => ToolError::Denied(reason),
        crate::approval::ApprovalError::TimedOut => ToolError::Denied("approval timed out".into()),
        crate::approval::ApprovalError::Internal(detail) => ToolError::MalformedArguments(detail),
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments(format!("missing or empty `{key}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::{ApprovalDecision, ApprovalError, ToolApprovalContext};
    use crate::fake_computer::FakeAgentComputer;
    use crate::subagent::{InMemoryAgentSubagents, SubagentTurn};
    use async_trait::async_trait;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    struct DenyAllGate;

    #[async_trait]
    impl ToolApprovalGate for DenyAllGate {
        async fn authorize(
            &self,
            _context: &ToolApprovalContext,
        ) -> Result<ApprovalDecision, ApprovalError> {
            Ok(ApprovalDecision::Deny {
                reason: "This Bot is not allowed to run subagents.".into(),
            })
        }
    }

    struct PanicTurn;

    #[async_trait]
    impl SubagentTurn for PanicTurn {
        async fn run_toolless(
            &self,
            _model: &str,
            _developer_instructions: &str,
            _user_prompt: &str,
            _cancel: &AtomicBool,
        ) -> Result<String, SubagentError> {
            panic!("subagent executor should not run");
        }
    }

    fn run_ctx() -> (ToolRunContext, CollaborationContext, AtomicBool) {
        (
            ToolRunContext {
                run_id: "run".into(),
                request_id: "req".into(),
                owner_id: "owner".into(),
                bot_id: "bot".into(),
                computer_id: "comp".into(),
                tool_invocation_id: Some("mcp:1".into()),
            },
            CollaborationContext {
                owner_id: "owner".into(),
                source_bot_id: "bot".into(),
                source_run_id: "run".into(),
                source_conversation_id: "conv".into(),
                source_request_id: "req".into(),
                tool_invocation_id: "mcp:1".into(),
            },
            AtomicBool::new(false),
        )
    }

    #[tokio::test]
    async fn deny_returns_clean_result_without_launching() {
        let computer = FakeAgentComputer::new();
        let subagents: Arc<dyn AgentSubagents> = Arc::new(InMemoryAgentSubagents::new());
        subagents.attach_turn_executor(Arc::new(PanicTurn));
        let (run, collab, cancel) = run_ctx();
        let result = dispatch_agent_tool_with_gate_and_recovery(
            &computer,
            None,
            None,
            None,
            Some(&subagents),
            None,
            None,
            None,
            None,
            None,
            None,
            "run_subagent",
            r#"{"name":"Reviewer","task":"check the plan"}"#,
            &cancel,
            &DenyAllGate,
            &run,
            Some(&collab),
            None,
        )
        .await
        .expect("deny is a model-facing result");
        assert_eq!(result["ok"], false);
        assert_eq!(result["status"], "denied");
        assert_eq!(result["detail"], "The helper was not launched.");
    }

    struct StubTurn;

    #[async_trait]
    impl SubagentTurn for StubTurn {
        async fn run_toolless(
            &self,
            _model: &str,
            _developer_instructions: &str,
            user_prompt: &str,
            _cancel: &AtomicBool,
        ) -> Result<String, SubagentError> {
            assert!(user_prompt.contains("check the plan"));
            Ok("looks good".into())
        }
    }

    #[tokio::test]
    async fn allow_runs_helper_and_returns_structured_result() {
        let computer = FakeAgentComputer::new();
        let subagents: Arc<dyn AgentSubagents> = Arc::new(InMemoryAgentSubagents::new());
        subagents.attach_turn_executor(Arc::new(StubTurn));
        let (run, collab, cancel) = run_ctx();
        let result = dispatch_agent_tool_with_gate_and_recovery(
            &computer,
            None,
            None,
            None,
            Some(&subagents),
            None,
            None,
            None,
            None,
            None,
            None,
            "run_subagent",
            r#"{"name":"Reviewer","task":"check the plan"}"#,
            &cancel,
            &crate::approval::AllowAllApprovalGate,
            &run,
            Some(&collab),
            None,
        )
        .await
        .expect("allow");
        assert_eq!(result["ok"], true);
        assert_eq!(result["status"], "completed");
        assert_eq!(result["result"], "looks good");
        assert_eq!(
            result["detail"],
            "This was a temporary helper for the current assignment, not another Bot."
        );
    }
}

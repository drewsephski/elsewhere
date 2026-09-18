use serde_json::{json, Value};

use crate::approval::{ToolApprovalContext, ToolApprovalGate, ToolRunContext};
use crate::skills::{
    is_skill_mutation_tool, AgentSkills, SkillContext, SkillError, SKILL_ATTACH_DESCRIPTION,
    SKILL_DETACH_TOOL_NAME, SKILL_LIST_DESCRIPTION, SKILL_LIST_TOOL_NAME,
    SKILL_SAVE_RECENT_WORK_DESCRIPTION, SKILL_SAVE_RECENT_WORK_TOOL_NAME,
    SKILL_ATTACH_TOOL_NAME, SKILL_DETACH_DESCRIPTION,
};
use crate::tool_catalog::is_skill_tool;
use crate::tools::ToolError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn skill_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": SKILL_LIST_TOOL_NAME,
            "description": SKILL_LIST_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": SKILL_SAVE_RECENT_WORK_TOOL_NAME,
            "description": SKILL_SAVE_RECENT_WORK_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Optional display name; slug is derived automatically" },
                    "description": { "type": "string", "description": "Optional short description for when to use this skill" },
                    "attachToBot": { "type": "boolean", "description": "Attach to this Bot after saving (default true)" }
                },
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": SKILL_ATTACH_TOOL_NAME,
            "description": SKILL_ATTACH_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "skillId": { "type": "string" }
                },
                "required": ["skillId"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": SKILL_DETACH_TOOL_NAME,
            "description": SKILL_DETACH_DESCRIPTION,
            "parameters": {
                "type": "object",
                "properties": {
                    "skillId": { "type": "string" }
                },
                "required": ["skillId"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub async fn dispatch_skill_tool(
    skills: Option<&Arc<dyn AgentSkills>>,
    name: &str,
    arguments: &str,
    cancel: &AtomicBool,
    gate: &dyn ToolApprovalGate,
    run: &ToolRunContext,
    skill_ctx: Option<&SkillContext>,
) -> Result<Value, ToolError> {
    if !is_skill_tool(name) {
        return Err(ToolError::MalformedArguments(format!(
            "unknown tool: {name}"
        )));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }
    let service = skills.ok_or_else(|| {
        ToolError::MalformedArguments("skill tools are not available in this run".into())
    })?;
    let ctx =
        skill_ctx.ok_or_else(|| ToolError::MalformedArguments("skill context missing".into()))?;

    let mut args: Value = serde_json::from_str(arguments)
        .map_err(|e| ToolError::MalformedArguments(format!("invalid JSON arguments: {e}")))?;

    if name == SKILL_SAVE_RECENT_WORK_TOOL_NAME {
        let attach = args
            .get("attachToBot")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let optional_name = args
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let optional_description = args
            .get("description")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let draft = service
            .prepare_save_from_recent_work(
                ctx,
                &run.run_id,
                optional_name,
                optional_description,
                attach,
            )
            .await
            .map_err(map_skill_error)?;
        let bot_name = service.bot_display_name(ctx).await.map_err(map_skill_error)?;
        let approval_args = json!({
            "name": draft.display_name,
            "slug": draft.slug,
            "description": draft.description,
            "attachToBot": draft.attach_to_bot,
            "botName": bot_name,
            "sourceRunId": draft.source_run_id,
            "skillMdPreview": truncate_skill_md_preview(&draft.skill_md),
            "draftKind": draft.draft_kind,
            "packageFileCount": draft.files.len(),
        });
        let approval_ctx =
            ToolApprovalContext::for_tool(run, SKILL_SAVE_RECENT_WORK_TOOL_NAME, approval_args);
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
        let saved = service
            .persist_save(ctx, &draft)
            .await
            .map_err(map_skill_error)?;
        return Ok(json!({
            "ok": true,
            "skill": {
                "id": saved.skill_id,
                "slug": saved.slug,
                "name": saved.name,
                "version": saved.version,
            },
            "attachedToBot": saved.attached_to_bot,
            "botName": saved.bot_name,
            "message": saved.message,
        }));
    }

    if name == SKILL_ATTACH_TOOL_NAME || name == SKILL_DETACH_TOOL_NAME {
        let skill_id = required_str(&args, "skillId")?;
        let rows = service.list(ctx).await.map_err(map_skill_error)?;
        let found = rows
            .iter()
            .find(|row| row.id == skill_id)
            .ok_or_else(|| ToolError::Denied("Skill not found for this owner".into()))?;
        if name == SKILL_ATTACH_TOOL_NAME {
            if found.status == "archived" {
                return Err(ToolError::MalformedArguments(
                    "Archived skills cannot be attached".into(),
                ));
            }
            if found.attached_to_bot {
                let bot_name = service.bot_display_name(ctx).await.map_err(map_skill_error)?;
                return Ok(json!({
                    "ok": true,
                    "skillId": found.id,
                    "slug": found.slug,
                    "name": found.name,
                    "botName": bot_name,
                    "message": format!("{} is already attached to {bot_name}", found.name),
                    "alreadyAttached": true,
                }));
            }
        } else if !found.attached_to_bot {
            return Err(ToolError::MalformedArguments(
                "This skill is not attached to this Bot".into(),
            ));
        }
        let bot_name = service.bot_display_name(ctx).await.map_err(map_skill_error)?;
        if let Some(obj) = args.as_object_mut() {
            obj.insert("skillName".into(), json!(found.name));
            obj.insert("skillSlug".into(), json!(found.slug));
            obj.insert("botName".into(), json!(bot_name));
        }
    }

    if is_skill_mutation_tool(name) && name != SKILL_SAVE_RECENT_WORK_TOOL_NAME {
        let approval_ctx = ToolApprovalContext::for_tool(run, name, args.clone());
        let approval = gate
            .authorize(&approval_ctx)
            .await
            .map_err(map_approval_error)?;
        if let crate::approval::ApprovalDecision::Deny { reason } = approval {
            return Err(ToolError::Denied(reason));
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(ToolError::Cancelled);
    }

    let result = match name {
        SKILL_LIST_TOOL_NAME => {
            let rows = service.list(ctx).await.map_err(map_skill_error)?;
            json!({ "ok": true, "skills": rows })
        }
        SKILL_ATTACH_TOOL_NAME => {
            let skill_id = required_str(&args, "skillId")?;
            let mutation = service.attach(ctx, skill_id).await.map_err(map_skill_error)?;
            json!({
                "ok": true,
                "skillId": mutation.skill_id,
                "slug": mutation.slug,
                "name": mutation.name,
                "botName": mutation.bot_name,
                "message": mutation.message,
            })
        }
        SKILL_DETACH_TOOL_NAME => {
            let skill_id = required_str(&args, "skillId")?;
            let mutation = service.detach(ctx, skill_id).await.map_err(map_skill_error)?;
            json!({
                "ok": true,
                "skillId": mutation.skill_id,
                "slug": mutation.slug,
                "name": mutation.name,
                "botName": mutation.bot_name,
                "message": mutation.message,
            })
        }
        other => {
            return Err(ToolError::MalformedArguments(format!(
                "unknown tool: {other}"
            )));
        }
    };

    Ok(result)
}

fn truncate_skill_md_preview(skill_md: &str) -> String {
    const MAX: usize = 480;
    if skill_md.len() <= MAX {
        return skill_md.to_string();
    }
    let mut end = MAX;
    while end > 0 && !skill_md.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &skill_md[..end])
}

#[cfg(test)]
mod tests {
    use super::truncate_skill_md_preview;

    #[test]
    fn preview_truncation_is_utf8_safe() {
        let emoji = "😀".repeat(200);
        let preview = truncate_skill_md_preview(&emoji);
        assert!(preview.ends_with('…'));
        assert!(std::str::from_utf8(preview.as_bytes()).is_ok());
    }

    #[test]
    fn preview_truncation_handles_accented_and_cjk() {
        let text = format!("{}café{}中文", "a".repeat(400), "b".repeat(100));
        let preview = truncate_skill_md_preview(&text);
        assert!(std::str::from_utf8(preview.as_bytes()).is_ok());
        assert!(preview.len() <= 481);
    }
}

fn required_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolError::MalformedArguments(format!("missing or empty `{key}`")))
}

fn map_approval_error(err: crate::approval::ApprovalError) -> ToolError {
    match err {
        crate::approval::ApprovalError::Cancelled => ToolError::Cancelled,
        crate::approval::ApprovalError::Denied { reason } => ToolError::Denied(reason),
        crate::approval::ApprovalError::TimedOut => ToolError::Denied("approval timed out".into()),
        crate::approval::ApprovalError::Internal(detail) => ToolError::MalformedArguments(detail),
    }
}

fn map_skill_error(err: SkillError) -> ToolError {
    match err {
        SkillError::NoSourceRun => ToolError::MalformedArguments(err.message()),
        SkillError::NotFound | SkillError::Forbidden => ToolError::Denied(err.message()),
        SkillError::Validation(m) | SkillError::Conflict(m) => ToolError::MalformedArguments(m),
        SkillError::Internal(m) => ToolError::MalformedArguments(m),
    }
}

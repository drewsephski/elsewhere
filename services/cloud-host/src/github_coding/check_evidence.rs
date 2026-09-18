use agent_core::GithubCodingError;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VerifiedCheck {
    pub command: String,
    pub exit_code: i32,
    pub ok: bool,
}

#[derive(Debug, Clone)]
struct ExecCall {
    call_id: String,
    command: String,
}

#[derive(Debug, Clone)]
struct ExecResult {
    call_id: String,
    exit_code: i32,
    ok: bool,
}

/// Pair durable `workspace_exec` tool_call / tool_result events for a request.
pub fn verify_check_commands_from_events(
    events: &[(String, Value)],
    requested_commands: &[String],
) -> Result<Vec<VerifiedCheck>, GithubCodingError> {
    if requested_commands.is_empty() {
        return Ok(Vec::new());
    }
    let mut calls: Vec<ExecCall> = Vec::new();
    let mut results: Vec<ExecResult> = Vec::new();
    for (event_type, payload) in events {
        if event_type != "tool_call" && event_type != "tool_result" {
            continue;
        }
        let tool = payload.get("tool").and_then(|v| v.as_str()).unwrap_or("");
        if tool != "workspace_exec" {
            continue;
        }
        let call_id = payload
            .get("callId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if call_id.is_empty() {
            continue;
        }
        if event_type == "tool_call" {
            let command = payload
                .get("arguments")
                .and_then(|a| a.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if command.is_empty() {
                continue;
            }
            calls.push(ExecCall { call_id, command });
            continue;
        }
        let exit_code = parse_exit_code_from_result(payload)?;
        let ok = payload.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) && exit_code == 0;
        results.push(ExecResult {
            call_id,
            exit_code,
            ok,
        });
    }

    let mut verified = Vec::new();
    for requested in requested_commands {
        let command = requested.trim();
        if command.is_empty() {
            return Err(GithubCodingError::Validation(
                "checkCommands entries must be non-empty".into(),
            ));
        }
        let matching_call = calls
            .iter()
            .filter(|c| c.command == command)
            .last()
            .ok_or_else(|| {
                GithubCodingError::Validation(format!(
                    "check command was not executed in this run: {command}"
                ))
            })?;
        let result = results
            .iter()
            .find(|r| r.call_id == matching_call.call_id)
            .ok_or_else(|| {
                GithubCodingError::Validation(format!(
                    "missing workspace_exec result for check command: {command}"
                ))
            })?;
        verified.push(VerifiedCheck {
            command: command.to_string(),
            exit_code: result.exit_code,
            ok: result.ok,
        });
    }
    Ok(verified)
}

fn parse_exit_code_from_result(payload: &Value) -> Result<i32, GithubCodingError> {
    if let Some(code) = payload.get("exitCode").and_then(|v| v.as_i64()) {
        return Ok(code as i32);
    }
    let output = payload.get("output").and_then(|v| v.as_str()).unwrap_or("");
    if output.is_empty() {
        return Ok(-1);
    }
    let parsed: Value = serde_json::from_str(output).map_err(|_| {
        GithubCodingError::Validation("workspace_exec result output was not valid JSON".into())
    })?;
    Ok(parsed
        .get("exitCode")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1) as i32)
}

pub fn reject_forged_check_fields(args: &Value) -> Result<(), GithubCodingError> {
    if args.get("checks").is_some() {
        return Err(GithubCodingError::Validation(
            "github_review_publish no longer accepts `checks`; use checkCommands and run workspace_exec first"
                .into(),
        ));
    }
    if args.get("exitCode").is_some() || args.get("ok").is_some() {
        return Err(GithubCodingError::Validation(
            "check results cannot be supplied by the model".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn forged_checks_field_rejected() {
        let err = reject_forged_check_fields(&json!({ "checks": [] })).expect_err("forged");
        assert!(matches!(err, GithubCodingError::Validation(_)));
    }

    #[test]
    fn recognizes_workspace_exec_result() {
        let events = vec![
            (
                "tool_call".into(),
                json!({
                    "tool": "workspace_exec",
                    "callId": "c1",
                    "arguments": { "command": "pnpm test" }
                }),
            ),
            (
                "tool_result".into(),
                json!({
                    "tool": "workspace_exec",
                    "callId": "c1",
                    "ok": true,
                    "output": r#"{"ok":true,"exitCode":0,"stdout":"","stderr":""}"#
                }),
            ),
        ];
        let verified = verify_check_commands_from_events(
            &events,
            &[String::from("pnpm test")],
        )
        .expect("verified");
        assert_eq!(verified.len(), 1);
        assert!(verified[0].ok);
    }

    #[test]
    fn unrun_command_rejected() {
        let events = vec![];
        let err = verify_check_commands_from_events(&events, &[String::from("pnpm test")])
            .expect_err("missing");
        assert!(matches!(err, GithubCodingError::Validation(_)));
    }
}

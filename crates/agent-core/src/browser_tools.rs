use serde_json::{json, Value};

use crate::computer::AgentComputer;
use crate::tool_catalog::is_browser_tool;
use crate::ToolError;

pub fn browser_openai_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "browser_navigate",
            "description": "Open a URL in the agent computer's headless browser (persistent session).",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Absolute http(s) URL" }
                },
                "required": ["url"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "browser_snapshot",
            "description": "Capture the current page URL, title, and an accessibility-style list of interactive elements with refs (e1, e2, …) for click/type.",
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "browser_click",
            "description": "Click an element identified by ref from browser_snapshot.",
            "parameters": {
                "type": "object",
                "properties": {
                    "ref": { "type": "string" }
                },
                "required": ["ref"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "browser_type",
            "description": "Type text into an input identified by ref from browser_snapshot.",
            "parameters": {
                "type": "object",
                "properties": {
                    "ref": { "type": "string" },
                    "text": { "type": "string" },
                    "submit": { "type": "boolean", "description": "Press Enter after typing" }
                },
                "required": ["ref", "text"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "browser_screenshot",
            "description": "Save a PNG screenshot of the current page under /workspace.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Output path under /workspace" },
                    "fullPage": { "type": "boolean" }
                },
                "required": ["path"],
                "additionalProperties": false
            },
            "strict": true
        }),
        json!({
            "type": "function",
            "name": "browser_download",
            "description": "Download a file from a URL into the workspace (uses browser network context).",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": { "type": "string" },
                    "path": { "type": "string", "description": "Destination path under /workspace" }
                },
                "required": ["url", "path"],
                "additionalProperties": false
            },
            "strict": true
        }),
    ]
}

pub async fn dispatch_browser_tool(
    computer: &dyn AgentComputer,
    name: &str,
    args: &Value,
) -> Result<Value, ToolError> {
    if !is_browser_tool(name) {
        return Err(ToolError::MalformedArguments(format!("not a browser tool: {name}")));
    }

    validate_browser_args(name, args).await?;

    let action = name.strip_prefix("browser_").unwrap_or(name);
    computer
        .browser_invoke(action, args)
        .await
        .map_err(ToolError::ComputerNotReady)
}

async fn validate_browser_args(name: &str, args: &Value) -> Result<(), ToolError> {
    match name {
        "browser_navigate" => {
            let url = required_str(args, "url")?;
            crate::public_http_url::validate_public_http_url(url).await?;
        }
        "browser_click" | "browser_type" => {
            required_str(args, "ref")?;
            if name == "browser_type" {
                required_str(args, "text")?;
            }
        }
        "browser_screenshot" | "browser_download" => {
            let path = required_str(args, "path")?;
            crate::workspace_entries::validate_workspace_mutation_path(path).map_err(|err| {
                ToolError::MalformedArguments(err.into())
            })?;
            if name == "browser_download" {
                let url = required_str(args, "url")?;
                crate::public_http_url::validate_public_http_url(url).await?;
            }
        }
        "browser_snapshot" => {}
        other => {
            return Err(ToolError::MalformedArguments(format!(
                "unknown browser tool: {other}"
            )));
        }
    }
    Ok(())
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
    use crate::computer::{ComputerError, ComputerInfo, ExecResult, WorkspaceEntry};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct BrowserFake {
        responses: Mutex<HashMap<String, Value>>,
    }

    #[async_trait]
    impl AgentComputer for BrowserFake {
        async fn ensure_ready(&self) -> Result<ComputerInfo, ComputerError> {
            Ok(ComputerInfo {
                ready: true,
                protocol_version: 1,
                detail: None,
            })
        }

        async fn list_dir(
            &self,
            _path: &str,
        ) -> Result<Vec<WorkspaceEntry>, ComputerError> {
            Ok(vec![])
        }

        async fn read_file(&self, _path: &str) -> Result<Vec<u8>, ComputerError> {
            Err(ComputerError::ExecutionFailed("no".into()))
        }

        async fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), ComputerError> {
            Ok(())
        }

        async fn exec(&self, _command: &str) -> Result<ExecResult, ComputerError> {
            Ok(ExecResult {
                ok: true,
                stdout: String::new(),
                stderr: String::new(),
                exit_code: 0,
            })
        }

        async fn browser_invoke(&self, action: &str, args: &Value) -> Result<Value, ComputerError> {
            let key = format!("{action}:{}", args.get("url").and_then(|v| v.as_str()).unwrap_or(""));
            if let Some(v) = self.responses.lock().unwrap().get(&key) {
                return Ok(v.clone());
            }
            Ok(json!({ "ok": true, "action": action }))
        }
    }

    #[tokio::test]
    async fn navigate_validates_url_scheme() {
        let computer = BrowserFake {
            responses: Mutex::new(HashMap::new()),
        };
        let err = dispatch_browser_tool(
            &computer,
            "browser_navigate",
            &json!({"url":"file:///etc/passwd"}),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::MalformedArguments(_)));

        let err = dispatch_browser_tool(
            &computer,
            "browser_navigate",
            &json!({"url":"http://127.0.0.1/"}),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::MalformedArguments(_)));
    }

    #[tokio::test]
    async fn screenshot_requires_workspace_path() {
        let computer = BrowserFake {
            responses: Mutex::new(HashMap::new()),
        };
        let err = dispatch_browser_tool(
            &computer,
            "browser_screenshot",
            &json!({"path":"/tmp/x.png"}),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::MalformedArguments(_)));

        let err = dispatch_browser_tool(
            &computer,
            "browser_screenshot",
            &json!({"path":"/workspace/../escape.png"}),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::MalformedArguments(_)));

        let err = dispatch_browser_tool(
            &computer,
            "browser_screenshot",
            &json!({"path":"/workspace/.elsewhere-bootstrap"}),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ToolError::MalformedArguments(_)));

        assert!(
            dispatch_browser_tool(
                &computer,
                "browser_screenshot",
                &json!({"path":"/workspace/screenshots/a.png"}),
            )
            .await
            .is_ok()
        );
    }
}

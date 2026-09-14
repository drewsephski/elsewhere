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

    validate_browser_args(name, args)?;

    let action = name.strip_prefix("browser_").unwrap_or(name);
    computer
        .browser_invoke(action, args)
        .await
        .map_err(ToolError::ComputerNotReady)
}

fn validate_browser_args(name: &str, args: &Value) -> Result<(), ToolError> {
    match name {
        "browser_navigate" => {
            let url = required_str(args, "url")?;
            validate_public_http_url(url)?;
        }
        "browser_click" | "browser_type" => {
            required_str(args, "ref")?;
            if name == "browser_type" {
                required_str(args, "text")?;
            }
        }
        "browser_screenshot" | "browser_download" => {
            let path = required_str(args, "path")?;
            require_workspace_path(path)?;
            if name == "browser_download" {
                let url = required_str(args, "url")?;
                validate_public_http_url(url)?;
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

fn validate_public_http_url(raw_url: &str) -> Result<(), ToolError> {
    if raw_url.len() > crate::approval::MAX_BROWSER_URL_CHARS {
        return Err(ToolError::MalformedArguments("url is too long".into()));
    }
    let parsed = url::Url::parse(raw_url).map_err(|_| {
        ToolError::MalformedArguments("url is not a valid http(s) URL".into())
    })?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(ToolError::MalformedArguments(
            "url must be http or https".into(),
        ));
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| ToolError::MalformedArguments("url is missing a host".into()))?
        .to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return Err(ToolError::MalformedArguments(
            "url targets a blocked host".into(),
        ));
    }
    if host == "169.254.169.254"
        || host == "metadata.google.internal"
        || host == "metadata.goog"
    {
        return Err(ToolError::MalformedArguments(
            "url targets a blocked host".into(),
        ));
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        let blocked = match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_unspecified()
                    || v4.is_multicast()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback()
                    || v6.is_unspecified()
                    || v6.is_multicast()
                    || v6.octets()[0] == 0xfc
                    || v6.octets()[0] == 0xfd
                    || (v6.octets()[0] == 0xfe && (v6.octets()[1] & 0xc0) == 0x80)
            }
        };
        if blocked {
            return Err(ToolError::MalformedArguments(
                "url targets a private or link-local address".into(),
            ));
        }
    }
    Ok(())
}

fn require_workspace_path(path: &str) -> Result<(), ToolError> {
    if path == "/workspace" || path.starts_with("/workspace/") {
        return Ok(());
    }
    Err(ToolError::MalformedArguments(format!(
        "path must be under /workspace, got {path}"
    )))
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
    }
}

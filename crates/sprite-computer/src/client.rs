use crate::policy::NetworkPolicyConfig;
use crate::types::{
    Checkpoint, CheckpointCreateBody, CheckpointRecord, CheckpointsListResponse,
    CreateSpriteBody, ExecHttpJson, FsListResponse, NetworkPolicyBody, SpriteError, SpriteInfo,
    SpriteRecord, StreamLine, DEFAULT_API_BASE,
};
use reqwest::{Client, Method, StatusCode};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct SpriteClientConfig {
    pub base_url: String,
    pub token: String,
    pub sprite_name: String,
    pub workspace_root: String,
    pub request_timeout: Duration,
    pub auto_create: bool,
    pub network_policy: NetworkPolicyConfig,
}

impl SpriteClientConfig {
    pub fn from_env_sprite_name(sprite_name: impl Into<String>) -> Result<Self, SpriteError> {
        let token = std::env::var("SPRITE_TOKEN")
            .or_else(|_| std::env::var("SPRITES_TOKEN"))
            .map_err(|_| {
                SpriteError::Config(
                    "SPRITE_TOKEN (or deprecated SPRITES_TOKEN) is not set".into(),
                )
            })?;
        Ok(Self {
            base_url: std::env::var("SPRITES_API_BASE").unwrap_or_else(|_| DEFAULT_API_BASE.into()),
            token,
            sprite_name: sprite_name.into(),
            workspace_root: std::env::var("ELSEWHERE_WORKSPACE_ROOT")
                .unwrap_or_else(|_| crate::types::DEFAULT_WORKSPACE_ROOT.into()),
            request_timeout: Duration::from_secs(120),
            auto_create: true,
            network_policy: crate::policy::default_deny_network_policy(),
        })
    }
}

pub struct SpriteClient {
    http: Client,
    config: SpriteClientConfig,
}

impl SpriteClient {
    pub fn new(config: SpriteClientConfig) -> Result<Self, SpriteError> {
        let http = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(config.request_timeout)
            .build()
            .map_err(|e| SpriteError::Config(e.to_string()))?;
        Ok(Self { http, config })
    }

    pub fn sprite_name(&self) -> &str {
        &self.config.sprite_name
    }

    pub fn workspace_root(&self) -> &str {
        &self.config.workspace_root
    }

    pub fn token(&self) -> &str {
        &self.config.token
    }

    pub async fn get_sprite(&self) -> Result<SpriteInfo, SpriteError> {
        let path = format!("/sprites/{}", urlencoding(&self.config.sprite_name));
        let record: SpriteRecord = self
            .send_json(Method::GET, &path, None::<&()>, true)
            .await?;
        Ok(record.into())
    }

    pub async fn sprite_exists(&self) -> Result<bool, SpriteError> {
        match self.get_sprite().await {
            Ok(_) => Ok(true),
            Err(SpriteError::NotFound) => Ok(false),
            Err(err) => Err(err),
        }
    }

    pub async fn create_sprite(&self) -> Result<SpriteInfo, SpriteError> {
        let body = CreateSpriteBody {
            name: &self.config.sprite_name,
        };
        let record: SpriteRecord = self
            .send_json(Method::POST, "/sprites", Some(body), false)
            .await?;
        self.set_network_policy(&self.config.network_policy).await?;
        Ok(record.into())
    }

    pub async fn destroy_sprite(&self) -> Result<(), SpriteError> {
        let path = format!("/sprites/{}", urlencoding(&self.config.sprite_name));
        self.send_empty(Method::DELETE, &path).await
    }

    pub async fn ensure_sprite(&self) -> Result<SpriteInfo, SpriteError> {
        if self.sprite_exists().await? {
            self.set_network_policy(&self.config.network_policy).await?;
            return self.get_sprite().await;
        }
        if !self.config.auto_create {
            return Err(SpriteError::NotFound);
        }
        info!(sprite_name = %self.config.sprite_name, "creating sprite");
        self.create_sprite().await
    }

    pub async fn set_network_policy(&self, policy: &NetworkPolicyConfig) -> Result<(), SpriteError> {
        let path = format!(
            "/sprites/{}/policy/network",
            urlencoding(&self.config.sprite_name)
        );
        self.send_json_body(Method::POST, &path, Some(policy.to_body()), false)
            .await?;
        Ok(())
    }

    pub async fn get_network_policy(&self) -> Result<NetworkPolicyBody, SpriteError> {
        let path = format!(
            "/sprites/{}/policy/network",
            urlencoding(&self.config.sprite_name)
        );
        self.send_json(Method::GET, &path, None::<&()>, true).await
    }

    pub(crate) async fn fs_list(&self, path: &str) -> Result<FsListResponse, SpriteError> {
        let path = format!(
            "/sprites/{}/fs/list?path={}&workingDir={}",
            urlencoding(&self.config.sprite_name),
            urlencoding(path),
            urlencoding(&self.config.workspace_root),
        );
        self.send_json(Method::GET, &path, None::<&()>, true).await
    }

    pub async fn fs_read(&self, path: &str) -> Result<Vec<u8>, SpriteError> {
        let path = format!(
            "/sprites/{}/fs/read?path={}&workingDir={}",
            urlencoding(&self.config.sprite_name),
            urlencoding(path),
            urlencoding(&self.config.workspace_root),
        );
        self.send_bytes(Method::GET, &path, None, true).await
    }

    pub async fn fs_write(&self, path: &str, data: &[u8], mkdir: bool) -> Result<(), SpriteError> {
        let path = format!(
            "/sprites/{}/fs/write?path={}&workingDir={}&mkdir={}",
            urlencoding(&self.config.sprite_name),
            urlencoding(path),
            urlencoding(&self.config.workspace_root),
            mkdir,
        );
        self.send_raw_with_timeout(Method::PUT, &path, Some(data), false, self.config.request_timeout)
            .await?;
        Ok(())
    }

    pub async fn exec_http(
        &self,
        command: &str,
        dir: &str,
        timeout: Duration,
    ) -> Result<(String, String, i32), SpriteError> {
        let path = format!(
            "/sprites/{}/exec?cmd=bash&cmd=-lc&cmd={}&dir={}",
            urlencoding(&self.config.sprite_name),
            urlencoding(command),
            urlencoding(dir),
        );
        let body = self
            .send_raw_with_timeout(Method::POST, &path, None, false, timeout)
            .await?;
        parse_exec_response(&body)
    }

    pub async fn create_checkpoint(&self, comment: Option<&str>) -> Result<Checkpoint, SpriteError> {
        let path = format!("/sprites/{}/checkpoint", urlencoding(&self.config.sprite_name));
        let body = CheckpointCreateBody { comment };
        let raw = self
            .send_ndjson(Method::POST, &path, Some(body))
            .await?;
        parse_checkpoint_complete(&raw)
    }

    pub async fn list_checkpoints(&self) -> Result<Vec<Checkpoint>, SpriteError> {
        let path = format!(
            "/sprites/{}/checkpoints",
            urlencoding(&self.config.sprite_name)
        );
        let list: CheckpointsListResponse = self
            .send_json(Method::GET, &path, None::<&()>, true)
            .await?;
        Ok(list
            .checkpoints
            .into_iter()
            .map(Checkpoint::from)
            .collect())
    }

    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<(), SpriteError> {
        let path = format!(
            "/sprites/{}/checkpoints/{}/restore",
            urlencoding(&self.config.sprite_name),
            urlencoding(checkpoint_id),
        );
        let raw = self.send_ndjson::<()>(Method::POST, &path, None).await?;
        for line in raw.lines() {
            if let Ok(event) = serde_json::from_str::<StreamLine>(line) {
                if event.r#type == "error" {
                    return Err(SpriteError::Provider {
                        status: 500,
                        message: event.error.unwrap_or_else(|| "restore failed".into()),
                    });
                }
            }
        }
        Ok(())
    }

    async fn send_json<T, B>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        retry_safe: bool,
    ) -> Result<T, SpriteError>
    where
        T: serde::de::DeserializeOwned,
        B: serde::Serialize,
    {
        let bytes = self.send_json_body(method, path, body, retry_safe).await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            SpriteError::MalformedResponse(SpriteError::sanitize_message(
                &e.to_string(),
                &self.config.token,
            ))
        })
    }

    async fn send_json_body<B>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
        retry_safe: bool,
    ) -> Result<Vec<u8>, SpriteError>
    where
        B: serde::Serialize,
    {
        let raw = match body {
            Some(value) => Some(
                serde_json::to_vec(&value)
                    .map_err(|e| SpriteError::Config(format!("json encode failed: {e}")))?,
            ),
            None => None,
        };
        self.send_raw_with_timeout(
            method,
            path,
            raw.as_deref(),
            retry_safe,
            self.config.request_timeout,
        )
        .await
    }

    async fn send_ndjson<B>(
        &self,
        method: Method,
        path: &str,
        body: Option<B>,
    ) -> Result<String, SpriteError>
    where
        B: serde::Serialize,
    {
        let bytes = self
            .send_json_body(method, path, body, false)
            .await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn send_empty(&self, method: Method, path: &str) -> Result<(), SpriteError> {
        let _ = self
            .send_raw_with_timeout(method, path, None, false, self.config.request_timeout)
            .await?;
        Ok(())
    }

    async fn send_bytes(
        &self,
        method: Method,
        path: &str,
        body: Option<&[u8]>,
        retry_safe: bool,
    ) -> Result<Vec<u8>, SpriteError> {
        self.send_raw_with_timeout(method, path, body, retry_safe, self.config.request_timeout)
            .await
    }

    async fn send_raw_with_timeout(
        &self,
        method: Method,
        path: &str,
        body: Option<&[u8]>,
        retry_safe: bool,
        timeout: Duration,
    ) -> Result<Vec<u8>, SpriteError> {
        let url = self.url(path)?;
        let operation = format!("{method} {path}");
        let mut attempts = if retry_safe { 3 } else { 1 };

        loop {
            attempts -= 1;
            let started = Instant::now();
            let mut req = self
                .http
                .request(method.clone(), url.clone())
                .header("Authorization", format!("Bearer {}", self.config.token))
                .timeout(timeout);

            if let Some(raw) = body {
                let content_type = if method == Method::PUT
                    && path.contains("/fs/write")
                {
                    "application/octet-stream"
                } else {
                    "application/json"
                };
                req = req.header("Content-Type", content_type).body(raw.to_vec());
            } else if method != Method::GET && method != Method::DELETE {
                req = req.header("Content-Type", "application/json");
            }

            let result = req.send().await;
            match result {
                Ok(response) => {
                    let status = response.status();
                    let request_id = response
                        .headers()
                        .get("x-request-id")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("")
                        .to_string();
                    debug!(
                        sprite_name = %self.config.sprite_name,
                        operation = %operation,
                        status_code = status.as_u16(),
                        duration_ms = started.elapsed().as_millis(),
                        request_id = %request_id,
                        "sprites api response"
                    );

                    if status == StatusCode::NOT_FOUND {
                        return Err(SpriteError::NotFound);
                    }

                    if status.is_success() {
                        let bytes = response.bytes().await.map_err(|e| map_network(e, &self.config.token))?;
                        if bytes.len() > MAX_RESPONSE_BYTES {
                            return Err(SpriteError::MalformedResponse(
                                "response body too large".into(),
                            ));
                        }
                        return Ok(bytes.to_vec());
                    }

                    let message = response
                        .text()
                        .await
                        .unwrap_or_else(|_| status.to_string());
                    let message = SpriteError::sanitize_message(&message, &self.config.token);

                    if retry_safe && status.is_server_error() && attempts > 0 {
                        warn!(
                            sprite_name = %self.config.sprite_name,
                            operation = %operation,
                            status_code = status.as_u16(),
                            "retrying sprites api request"
                        );
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }

                    return Err(SpriteError::Provider {
                        status: status.as_u16(),
                        message,
                    });
                }
                Err(err) => {
                    if err.is_timeout() {
                        return Err(SpriteError::Timeout);
                    }
                    if retry_safe && attempts > 0 {
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                    return Err(map_network(err, &self.config.token));
                }
            }
        }
    }

    fn url(&self, path: &str) -> Result<url::Url, SpriteError> {
        let base = self.config.base_url.trim_end_matches('/');
        url::Url::parse(&format!("{base}{path}"))
            .map_err(|e| SpriteError::Config(format!("invalid base url: {e}")))
    }
}

impl From<SpriteRecord> for SpriteInfo {
    fn from(value: SpriteRecord) -> Self {
        Self {
            id: value.id,
            name: value.name,
            organization: value.organization,
            status: value.status,
        }
    }
}

impl From<CheckpointRecord> for Checkpoint {
    fn from(value: CheckpointRecord) -> Self {
        Self {
            id: value.id,
            comment: value.comment,
            created_at: value.created_at,
        }
    }
}

fn map_network(err: reqwest::Error, token: &str) -> SpriteError {
    SpriteError::Network(SpriteError::sanitize_message(
        &err.to_string(),
        token,
    ))
}

fn urlencoding(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub(crate) fn parse_exec_response(body: &[u8]) -> Result<(String, String, i32), SpriteError> {
    if body.is_empty() {
        return Ok((String::new(), String::new(), 0));
    }
    if body.first() == Some(&b'{') {
        let parsed: ExecHttpJson = serde_json::from_slice(body).map_err(|e| {
            SpriteError::MalformedResponse(format!("exec json parse failed: {e}"))
        })?;
        return Ok((parsed.stdout, parsed.stderr, parsed.exit_code));
    }
    parse_exec_binary(body)
}

fn parse_exec_binary(body: &[u8]) -> Result<(String, String, i32), SpriteError> {
    let mut idx = 0usize;
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code = 0i32;

    while idx < body.len() {
        let stream = body[idx];
        idx += 1;
        match stream {
            1 => {
                let rest = &body[idx..];
                let end = rest
                    .iter()
                    .position(|&b| b == 2 || b == 3)
                    .unwrap_or(rest.len());
                stdout = String::from_utf8_lossy(&rest[..end]).into_owned();
                idx += end;
            }
            2 => {
                let rest = &body[idx..];
                let end = rest.iter().position(|&b| b == 3).unwrap_or(rest.len());
                stderr = String::from_utf8_lossy(&rest[..end]).into_owned();
                idx += end;
            }
            3 => {
                if idx < body.len() {
                    exit_code = body[idx] as i32;
                }
                break;
            }
            other => {
                return Err(SpriteError::MalformedResponse(format!(
                    "unknown exec stream id {other}"
                )));
            }
        }
    }

    Ok((stdout, stderr, exit_code))
}

fn parse_checkpoint_complete(ndjson: &str) -> Result<Checkpoint, SpriteError> {
    let mut last_error = None;
    for line in ndjson.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let event: StreamLine = serde_json::from_str(line).map_err(|e| {
            SpriteError::MalformedResponse(format!("checkpoint stream parse failed: {e}"))
        })?;
        match event.r#type.as_str() {
            "error" => {
                last_error = Some(event.error.unwrap_or_else(|| "checkpoint failed".into()));
            }
            "complete" => {
                let data = event.data.unwrap_or_default();
                let id = data
                    .split_whitespace()
                    .find(|part| part.starts_with('v'))
                    .unwrap_or("unknown")
                    .trim_end_matches(|c: char| !c.is_ascii_alphanumeric())
                    .to_string();
                return Ok(Checkpoint {
                    id,
                    comment: None,
                    created_at: None,
                });
            }
            _ => {}
        }
    }
    if let Some(err) = last_error {
        return Err(SpriteError::Provider {
            status: 500,
            message: err,
        });
    }
    Err(SpriteError::MalformedResponse(
        "checkpoint stream ended without complete event".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite_name_for_sandbox;

    #[test]
    fn exec_json_response_parses() {
        let body = br#"{"stdout":"ok","stderr":"","exitCode":0}"#;
        let (stdout, stderr, code) = parse_exec_response(body).unwrap();
        assert_eq!(stdout, "ok");
        assert_eq!(stderr, "");
        assert_eq!(code, 0);
    }

    #[test]
    fn exec_binary_response_parses() {
        let body = vec![1, b'h', b'i', 3, 0];
        let (stdout, _, code) = parse_exec_response(&body).unwrap();
        assert_eq!(stdout, "hi");
        assert_eq!(code, 0);
    }

    #[test]
    fn sanitize_removes_token() {
        let msg = "Authorization: Bearer secret-token failed";
        let out = SpriteError::sanitize_message(msg, "secret-token");
        assert!(!out.contains("secret-token"));
    }

    #[test]
    fn sprite_name_for_sandbox_is_stable() {
        assert_eq!(
            sprite_name_for_sandbox("01KABC_test"),
            "elsewhere-01kabc-test"
        );
    }
}

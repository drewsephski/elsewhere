use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const INSTALLATION_PAGE_SIZE: u32 = 100;
const REPO_PAGE_SIZE: u32 = 100;
const MAX_INSTALLATIONS: usize = 50;
const MAX_AUTHORIZED_REPOS: usize = 1000;

#[derive(Debug, Clone)]
pub struct GitHubTreeChange {
    pub path: String,
    pub mode: String,
    pub deleted: bool,
    pub content: Option<Vec<u8>>,
}

#[derive(Clone)]
pub struct GitHubClient {
    http: reqwest::Client,
    api_base: String,
    oauth_base: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

impl Default for GitHubClient {
    fn default() -> Self {
        Self::production()
    }
}

impl GitHubClient {
    pub fn production() -> Self {
        Self {
            http: reqwest::Client::new(),
            api_base: "https://api.github.com".into(),
            oauth_base: "https://github.com".into(),
            client_id: None,
            client_secret: None,
        }
    }

    pub fn with_api_base(api_base: String, oauth_base: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_base,
            oauth_base,
            client_id: None,
            client_secret: None,
        }
    }

    pub fn with_oauth(mut self, client_id: String, client_secret: String) -> Self {
        self.client_id = Some(client_id);
        self.client_secret = Some(client_secret);
        self
    }

    pub fn installation_url(&self, app_slug: &str, state: &str) -> String {
        format!(
            "{}/apps/{}/installations/new?state={}",
            self.oauth_base.trim_end_matches('/'),
            urlencoding::encode(app_slug),
            urlencoding::encode(state),
        )
    }

    pub async fn exchange_code(
        &self,
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<GitHubAppUserToken, String> {
        let payload = json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "code": code,
            "redirect_uri": redirect_uri,
        });
        self.request_user_token(payload).await
    }

    pub async fn refresh_user_token(
        &self,
        refresh_token: &str,
    ) -> Result<GitHubAppUserToken, GitHubRefreshError> {
        let client_id = self
            .client_id
            .as_deref()
            .ok_or(GitHubRefreshError::NotConfigured)?;
        let client_secret = self
            .client_secret
            .as_deref()
            .ok_or(GitHubRefreshError::NotConfigured)?;
        let payload = json!({
            "client_id": client_id,
            "client_secret": client_secret,
            "grant_type": "refresh_token",
            "refresh_token": refresh_token,
        });
        match self.request_user_token(payload).await {
            Ok(token) => Ok(token),
            Err(message)
                if message.contains("token exchange failed")
                    || message.contains("API request failed") =>
            {
                Err(GitHubRefreshError::Provider(message))
            }
            Err(_) => Err(GitHubRefreshError::InvalidGrant),
        }
    }

    pub async fn get_user(&self, token: &str) -> Result<GitHubUser, String> {
        self.get_json(token, "/user").await
    }

    pub async fn list_user_installations(
        &self,
        token: &str,
    ) -> Result<Vec<GitHubInstallation>, String> {
        let mut installations = Vec::new();
        let mut page = 1u32;
        loop {
            let path = format!("/user/installations?per_page={INSTALLATION_PAGE_SIZE}&page={page}");
            let body: GitHubInstallationsResponse = self.get_json(token, &path).await?;
            let batch_len = body.installations.len();
            for installation in body.installations {
                if installations.len() >= MAX_INSTALLATIONS {
                    break;
                }
                installations.push(installation);
            }
            if installations.len() >= MAX_INSTALLATIONS
                || batch_len < INSTALLATION_PAGE_SIZE as usize
            {
                break;
            }
            page += 1;
        }
        Ok(installations)
    }

    pub async fn list_installation_repositories(
        &self,
        token: &str,
        installation_id: i64,
    ) -> Result<Vec<Value>, String> {
        let mut repositories = Vec::new();
        let mut page = 1u32;
        loop {
            let path = format!(
                "/user/installations/{installation_id}/repositories?per_page={REPO_PAGE_SIZE}&page={page}"
            );
            let body: GitHubInstallationRepositoriesResponse = self.get_json(token, &path).await?;
            let batch_len = body.repositories.len();
            repositories.extend(body.repositories);
            if repositories.len() >= MAX_AUTHORIZED_REPOS || batch_len < REPO_PAGE_SIZE as usize {
                repositories.truncate(MAX_AUTHORIZED_REPOS);
                break;
            }
            page += 1;
        }
        Ok(repositories)
    }

    pub async fn get_repository(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> Result<Value, String> {
        let path = format!("/repos/{}/{}", owner, repo);
        self.get_json(token, &path).await
    }

    pub async fn get_file_contents(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        path: &str,
        git_ref: Option<&str>,
    ) -> Result<Value, String> {
        let mut path = format!(
            "/repos/{}/{}/contents/{}",
            owner,
            repo,
            path.trim_start_matches('/')
        );
        if let Some(r) = git_ref.filter(|s| !s.is_empty()) {
            path.push_str(&format!("?ref={}", urlencoding::encode(r)));
        }
        let value: Value = self.get_json(token, &path).await?;
        if let Some(encoding) = value.get("encoding").and_then(|v| v.as_str()) {
            if encoding == "base64" {
                if let Some(content) = value.get("content").and_then(|v| v.as_str()) {
                    let cleaned = content.replace('\n', "");
                    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(cleaned) {
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        return Ok(json!({
                            "ok": true,
                            "path": value.get("path"),
                            "sha": value.get("sha"),
                            "size": value.get("size"),
                            "content": text,
                            "encoding": "utf-8"
                        }));
                    }
                }
            }
        }
        Ok(json!({ "ok": true, "entry": value }))
    }

    pub async fn list_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        per_page: u32,
        page: u32,
    ) -> Result<Value, String> {
        let path = format!(
            "/repos/{}/{}/issues?state={}&per_page={}&page={}",
            owner,
            repo,
            state,
            per_page.clamp(1, 100),
            page.max(1)
        );
        self.get_json(token, &path).await
    }

    pub async fn get_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Value, String> {
        let path = format!("/repos/{}/{}/issues/{}", owner, repo, number);
        self.get_json(token, &path).await
    }

    pub async fn list_pull_requests(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        per_page: u32,
        page: u32,
    ) -> Result<Value, String> {
        let path = format!(
            "/repos/{}/{}/pulls?state={}&per_page={}&page={}",
            owner,
            repo,
            state,
            per_page.clamp(1, 100),
            page.max(1)
        );
        self.get_json(token, &path).await
    }

    pub async fn get_pull_request(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> Result<Value, String> {
        let path = format!("/repos/{}/{}/pulls/{}", owner, repo, number);
        self.get_json(token, &path).await
    }

    pub async fn download_tarball(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        git_ref: &str,
    ) -> Result<Vec<u8>, String> {
        let path = format!(
            "/repos/{}/{}/tarball/{}",
            urlencoding::encode(owner),
            urlencoding::encode(repo),
            urlencoding::encode(git_ref),
        );
        let url = format!("{}{}", self.api_base, path);
        let response = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| format!("GitHub zipball download failed: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("GitHub zipball error {}", response.status()));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("GitHub zipball body invalid: {e}"))?;
        Ok(bytes.to_vec())
    }

    pub async fn get_commit_tree_sha(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        commit_sha: &str,
    ) -> Result<String, String> {
        let path = format!("/repos/{}/{}/git/commits/{}", owner, repo, commit_sha);
        let body: Value = self.get_json(token, &path).await?;
        body.get("tree")
            .and_then(|t| t.get("sha"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "GitHub commit response missing tree sha".into())
    }

    pub async fn get_branch_head_sha(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<String, String> {
        let path = format!(
            "/repos/{}/{}/git/ref/heads/{}",
            owner,
            repo,
            urlencoding::encode(branch),
        );
        let body: Value = self.get_json(token, &path).await?;
        body.get("object")
            .and_then(|o| o.get("sha"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "GitHub ref response missing sha".into())
    }

    pub async fn find_open_pull_for_head(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        head_branch: &str,
    ) -> Result<Option<Value>, String> {
        let head = format!("{}:{}", owner, head_branch);
        let path = format!(
            "/repos/{}/{}/pulls?state=open&head={}&per_page=5",
            owner,
            repo,
            urlencoding::encode(&head),
        );
        let pulls: Vec<Value> = self.get_json(token, &path).await?;
        Ok(pulls.into_iter().next())
    }

    pub async fn create_pull_request(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        title: &str,
        body: &str,
        head_branch: &str,
        base_branch: &str,
    ) -> Result<Value, String> {
        let path = format!("/repos/{}/{}/pulls", owner, repo);
        self.post_json(
            token,
            &path,
            &json!({
                "title": title,
                "body": body,
                "head": head_branch,
                "base": base_branch,
            }),
        )
        .await
    }

    pub async fn ref_head_sha(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<Option<String>, String> {
        let path = format!(
            "/repos/{}/{}/git/ref/heads/{}",
            owner,
            repo,
            urlencoding::encode(branch),
        );
        let body: Result<Value, String> = self.get_json(token, &path).await;
        match body {
            Ok(value) => Ok(value
                .get("object")
                .and_then(|o| o.get("sha"))
                .and_then(|s| s.as_str())
                .map(|s| s.to_string())),
            Err(message) if message.contains("404") => Ok(None),
            Err(message) => Err(message),
        }
    }

    pub async fn publish_file_changes(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        base_commit_sha: &str,
        base_tree_sha: &str,
        branch: &str,
        message: &str,
        changes: &[GitHubTreeChange],
        expected_existing_commit: Option<&str>,
    ) -> Result<String, String> {
        let existing_ref = self.ref_head_sha(token, owner, repo, branch).await?;
        if let Some(head) = existing_ref {
            if let Some(expected) = expected_existing_commit {
                if head != expected {
                    return Err(
                        "working branch already exists for another Elsewhere change".into(),
                    );
                }
                return Ok(head);
            }
            if self
                .commit_matches_prepared_changes(token, owner, repo, &head, base_commit_sha, changes)
                .await?
            {
                return Ok(head);
            }
            return Err("working branch already exists for another Elsewhere change".into());
        }

        let mut tree_items = Vec::new();
        for change in changes {
            if change.deleted {
                tree_items.push(json!({
                    "path": change.path,
                    "mode": change.mode,
                    "type": "blob",
                    "sha": null
                }));
                continue;
            }
            let content = change
                .content
                .as_ref()
                .ok_or_else(|| format!("missing content for {}", change.path))?;
            let blob_body = if std::str::from_utf8(content).is_err() {
                json!({
                    "content": base64::engine::general_purpose::STANDARD.encode(content),
                    "encoding": "base64"
                })
            } else {
                json!({
                    "content": String::from_utf8_lossy(content),
                    "encoding": "utf-8"
                })
            };
            let blob: Value = self
                .post_json(
                    token,
                    &format!("/repos/{}/{}/git/blobs", owner, repo),
                    &blob_body,
                )
                .await?;
            let sha = blob
                .get("sha")
                .and_then(|s| s.as_str())
                .ok_or_else(|| "blob missing sha".to_string())?;
            tree_items.push(json!({
                "path": change.path,
                "mode": change.mode,
                "type": "blob",
                "sha": sha
            }));
        }
        let tree: Value = self
            .post_json(
                token,
                &format!("/repos/{}/{}/git/trees", owner, repo),
                &json!({
                    "base_tree": base_tree_sha,
                    "tree": tree_items
                }),
            )
            .await?;
        let tree_sha = tree
            .get("sha")
            .and_then(|s| s.as_str())
            .ok_or_else(|| "tree missing sha".to_string())?;
        let commit: Value = self
            .post_json(
                token,
                &format!("/repos/{}/{}/git/commits", owner, repo),
                &json!({
                    "message": message,
                    "tree": tree_sha,
                    "parents": [base_commit_sha]
                }),
            )
            .await?;
        let commit_sha = commit
            .get("sha")
            .and_then(|s| s.as_str())
            .ok_or_else(|| "commit missing sha".to_string())?;

        let _: Value = self
            .post_json(
                token,
                &format!("/repos/{}/{}/git/refs", owner, repo),
                &json!({
                    "ref": format!("refs/heads/{branch}"),
                    "sha": commit_sha
                }),
            )
            .await?;
        Ok(commit_sha.to_string())
    }

    async fn commit_matches_prepared_changes(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        commit_sha: &str,
        base_commit_sha: &str,
        changes: &[GitHubTreeChange],
    ) -> Result<bool, String> {
        let path = format!("/repos/{}/{}/git/commits/{}", owner, repo, commit_sha);
        let commit: Value = self.get_json(token, &path).await?;
        let parents = commit
            .get("parents")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "commit missing parents".to_string())?;
        let parent = parents
            .first()
            .and_then(|p| p.get("sha"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| "commit missing parent sha".to_string())?;
        if parent != base_commit_sha {
            return Ok(false);
        }
        let tree_sha = commit
            .get("tree")
            .and_then(|t| t.get("sha"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| "commit missing tree sha".to_string())?;
        let tree_path = format!(
            "/repos/{}/{}/git/trees/{}?recursive=1",
            owner,
            repo,
            tree_sha
        );
        let tree: Value = self.get_json(token, &tree_path).await?;
        let entries = tree
            .get("tree")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "tree missing entries".to_string())?;
        for change in changes {
            if change.deleted {
                if entries.iter().any(|e| e.get("path").and_then(|p| p.as_str()) == Some(&change.path)) {
                    return Ok(false);
                }
                continue;
            }
            let content = change
                .content
                .as_ref()
                .ok_or_else(|| format!("missing content for {}", change.path))?;
            let expected_blob = self
                .create_blob_sha(token, owner, repo, content)
                .await?;
            let actual = entries
                .iter()
                .find(|e| e.get("path").and_then(|p| p.as_str()) == Some(&change.path))
                .and_then(|e| e.get("sha"))
                .and_then(|s| s.as_str());
            if actual != Some(expected_blob.as_str()) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn create_blob_sha(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        content: &[u8],
    ) -> Result<String, String> {
        let blob_body = if std::str::from_utf8(content).is_err() {
            json!({
                "content": base64::engine::general_purpose::STANDARD.encode(content),
                "encoding": "base64"
            })
        } else {
            json!({
                "content": String::from_utf8_lossy(content),
                "encoding": "utf-8"
            })
        };
        let blob: Value = self
            .post_json(
                token,
                &format!("/repos/{}/{}/git/blobs", owner, repo),
                &blob_body,
            )
            .await?;
        blob.get("sha")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "blob missing sha".to_string())
    }

    async fn post_json<T: for<'de> Deserialize<'de>>(
        &self,
        token: &str,
        path: &str,
        body: &Value,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.api_base, path);
        let response = self
            .http
            .post(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err("GitHub rejected the stored credentials".into());
        }
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let detail = crate::redact::redact_secrets(&detail);
            return Err(format!("GitHub API error {status}: {detail}"));
        }
        response
            .json()
            .await
            .map_err(|e| format!("GitHub API response invalid: {e}"))
    }

    async fn patch_json<T: for<'de> Deserialize<'de>>(
        &self,
        token: &str,
        path: &str,
        body: &Value,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.api_base, path);
        let response = self
            .http
            .patch(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let detail = crate::redact::redact_secrets(&detail);
            return Err(format!("GitHub API error {status}: {detail}"));
        }
        response
            .json()
            .await
            .map_err(|e| format!("GitHub API response invalid: {e}"))
    }

    async fn request_user_token(&self, payload: Value) -> Result<GitHubAppUserToken, String> {
        let response = self
            .http
            .post(format!(
                "{}/login/oauth/access_token",
                self.oauth_base.trim_end_matches('/')
            ))
            .header("Accept", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("GitHub token exchange failed: {e}"))?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::BAD_REQUEST
        {
            return Err("GitHub rejected the stored credentials".into());
        }
        if !status.is_success() {
            return Err(format!("GitHub token exchange returned {status}"));
        }

        let body: GitHubTokenResponse = response
            .json()
            .await
            .map_err(|_| "GitHub token response invalid".to_string())?;
        body.into_user_token()
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        token: &str,
        path: &str,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.api_base, path);
        let response = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("Bearer {token}"))
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err("GitHub rejected the stored credentials".into());
        }
        if !response.status().is_success() {
            let status = response.status();
            let detail = response.text().await.unwrap_or_default();
            let detail = crate::redact::redact_secrets(&detail);
            return Err(format!("GitHub API error {status}: {detail}"));
        }

        response
            .json()
            .await
            .map_err(|e| format!("GitHub API response invalid: {e}"))
    }
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: Option<String>,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
    refresh_token_expires_in: Option<i64>,
    token_type: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

impl GitHubTokenResponse {
    fn into_user_token(self) -> Result<GitHubAppUserToken, String> {
        if let Some(err) = self.error {
            return Err(self.error_description.unwrap_or(err));
        }
        let access_token = self
            .access_token
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "GitHub did not return an access token".to_string())?;
        let refresh_token = self
            .refresh_token
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "GitHub did not return a refresh token".to_string())?;
        let expires_in = self
            .expires_in
            .filter(|n| *n > 0)
            .ok_or_else(|| "GitHub token response missing expires_in".to_string())?;
        let refresh_token_expires_in = self
            .refresh_token_expires_in
            .filter(|n| *n > 0)
            .ok_or_else(|| "GitHub token response missing refresh_token_expires_in".to_string())?;
        Ok(GitHubAppUserToken {
            access_token,
            refresh_token,
            expires_in,
            refresh_token_expires_in,
            token_type: self
                .token_type
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "bearer".into()),
            scope: self.scope.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubAppUserToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub refresh_token_expires_in: i64,
    pub token_type: String,
    pub scope: String,
}

impl GitHubAppUserToken {
    pub fn into_credential(self, now: DateTime<Utc>) -> GitHubCredential {
        GitHubCredential::from_user_token(self, now)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubRefreshError {
    NotConfigured,
    InvalidGrant,
    Provider(String),
}

const GITHUB_CREDENTIAL_KIND: &str = "github_app_user";
pub const ACCESS_TOKEN_SAFETY_WINDOW_SECS: i64 = 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GitHubCredential {
    pub kind: String,
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    pub token_type: String,
}

impl GitHubCredential {
    pub fn from_user_token(token: GitHubAppUserToken, now: DateTime<Utc>) -> Self {
        Self {
            kind: GITHUB_CREDENTIAL_KIND.into(),
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            access_expires_at: now + Duration::seconds(token.expires_in),
            refresh_expires_at: now + Duration::seconds(token.refresh_token_expires_in),
            token_type: token.token_type,
        }
    }

    pub fn from_plaintext(plaintext: &str) -> Option<Self> {
        let parsed: Self = serde_json::from_str(plaintext).ok()?;
        if parsed.kind != GITHUB_CREDENTIAL_KIND
            || parsed.access_token.is_empty()
            || parsed.refresh_token.is_empty()
        {
            return None;
        }
        Some(parsed)
    }

    pub fn to_plaintext(&self) -> Result<String, String> {
        serde_json::to_string(self).map_err(|e| e.to_string())
    }

    pub fn access_token_is_fresh(&self, now: DateTime<Utc>) -> bool {
        self.access_expires_at > now + Duration::seconds(ACCESS_TOKEN_SAFETY_WINDOW_SECS)
    }

    pub fn refresh_token_is_valid(&self, now: DateTime<Utc>) -> bool {
        self.refresh_expires_at > now
    }

    pub fn secret_values(&self) -> Vec<String> {
        vec![self.access_token.clone(), self.refresh_token.clone()]
    }
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct GitHubUser {
    pub login: String,
    pub id: i64,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubInstallationsResponse {
    #[serde(default)]
    installations: Vec<GitHubInstallation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubInstallation {
    pub id: i64,
    pub account: GitHubInstallationAccount,
    #[serde(default)]
    pub repository_selection: String,
    #[serde(default)]
    pub permissions: Value,
}

impl GitHubInstallation {
    pub fn metadata_summary(&self) -> Value {
        json!({
            "id": self.id,
            "accountLogin": self.account.login,
            "accountId": self.account.id,
            "accountType": self.account.account_type,
            "repositorySelection": self.repository_selection,
            "permissionsSummary": permissions_summary(&self.permissions),
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubInstallationAccount {
    pub login: String,
    pub id: i64,
    #[serde(rename = "type")]
    pub account_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubInstallationRepositoriesResponse {
    #[serde(default)]
    repositories: Vec<Value>,
}

fn permissions_summary(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, perm) in map {
                if let Some(level) = perm.as_str() {
                    out.insert(key.clone(), json!(level));
                }
            }
            Value::Object(out)
        }
        _ => json!({}),
    }
}

pub fn parse_github_app_token_response_json(value: &Value) -> Result<GitHubAppUserToken, String> {
    let parsed: GitHubTokenResponse = serde_json::from_value(value.clone())
        .map_err(|_| "GitHub token response invalid".to_string())?;
    parsed.into_user_token()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn installation_url_uses_app_slug_and_state_without_classic_scopes() {
        let client = GitHubClient::production();
        let url = client.installation_url("elsewhere-alpha", "state-token");
        assert_eq!(
            url,
            "https://github.com/apps/elsewhere-alpha/installations/new?state=state-token"
        );
        assert!(!url.contains("scope="));
        assert!(!url.contains("repo"));
        assert!(!url.contains("login/oauth/authorize"));
    }

    #[test]
    fn token_response_requires_refreshable_github_app_fields() {
        let token = parse_github_app_token_response_json(&json!({
            "access_token": "ghu_access",
            "expires_in": 28800,
            "refresh_token": "ghr_refresh",
            "refresh_token_expires_in": 15897600,
            "token_type": "bearer",
            "scope": ""
        }))
        .expect("token");
        assert_eq!(token.access_token, "ghu_access");
        assert_eq!(token.refresh_token, "ghr_refresh");
        assert_eq!(token.expires_in, 28800);
        assert_eq!(token.refresh_token_expires_in, 15897600);
    }

    #[test]
    fn legacy_access_token_only_response_is_rejected() {
        let err = parse_github_app_token_response_json(&json!({
            "access_token": "gho_legacy",
            "token_type": "bearer",
            "scope": "repo"
        }))
        .expect_err("legacy");
        assert!(err.contains("refresh token") || err.contains("expires_in"));
    }

    #[test]
    fn structured_credential_round_trips_and_rejects_raw_tokens() {
        let token = GitHubAppUserToken {
            access_token: "ghu_access".into(),
            refresh_token: "ghr_refresh".into(),
            expires_in: 100,
            refresh_token_expires_in: 200,
            token_type: "bearer".into(),
            scope: String::new(),
        };
        let now = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let credential = GitHubCredential::from_user_token(token, now);
        let plaintext = credential.to_plaintext().unwrap();
        assert!(GitHubCredential::from_plaintext(&plaintext).is_some());
        assert!(GitHubCredential::from_plaintext("gho_legacy_raw_token").is_none());
        assert_eq!(credential.access_expires_at, now + Duration::seconds(100));
        assert_eq!(credential.refresh_expires_at, now + Duration::seconds(200));
    }
}

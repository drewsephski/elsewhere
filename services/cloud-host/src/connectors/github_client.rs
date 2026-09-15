use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct GitHubClient {
    http: reqwest::Client,
    api_base: String,
    oauth_base: String,
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
        }
    }

    pub fn with_api_base(api_base: String, oauth_base: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_base,
            oauth_base,
        }
    }

    pub fn authorize_url(
        &self,
        client_id: &str,
        redirect_uri: &str,
        state: &str,
    ) -> String {
        format!(
            "{}/login/oauth/authorize?client_id={}&redirect_uri={}&scope={}&state={}",
            self.oauth_base,
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode("read:user,repo"),
            urlencoding::encode(state),
        )
    }

    pub async fn exchange_code(
        &self,
        client_id: &str,
        client_secret: &str,
        code: &str,
        redirect_uri: &str,
    ) -> Result<String, String> {
        let response = self
            .http
            .post(format!("{}/login/oauth/access_token", self.oauth_base))
            .header("Accept", "application/json")
            .json(&json!({
                "client_id": client_id,
                "client_secret": client_secret,
                "code": code,
                "redirect_uri": redirect_uri,
            }))
            .send()
            .await
            .map_err(|e| format!("GitHub token exchange failed: {e}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "GitHub token exchange returned {}",
                response.status()
            ));
        }

        let body: OAuthTokenResponse = response
            .json()
            .await
            .map_err(|e| format!("GitHub token response invalid: {e}"))?;
        if let Some(err) = body.error {
            return Err(body.error_description.unwrap_or(err));
        }
        body.access_token
            .filter(|t| !t.is_empty())
            .ok_or_else(|| "GitHub did not return an access token".into())
    }

    pub async fn get_user(&self, token: &str) -> Result<GitHubUser, String> {
        self.get_json(token, "/user").await
    }

    pub async fn list_repositories(
        &self,
        token: &str,
        visibility: &str,
        per_page: u32,
        page: u32,
    ) -> Result<Value, String> {
        let path = format!(
            "/user/repos?visibility={}&sort=updated&per_page={}&page={}",
            visibility,
            per_page.clamp(1, 100),
            page.max(1)
        );
        self.get_json(token, &path).await
    }

    pub async fn search_repositories(
        &self,
        token: &str,
        query: &str,
        per_page: u32,
        page: u32,
    ) -> Result<Value, String> {
        let path = format!(
            "/search/repositories?q={}&per_page={}&page={}",
            urlencoding::encode(query),
            per_page.clamp(1, 100),
            page.max(1)
        );
        self.get_json(token, &path).await
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
        let mut path = format!("/repos/{}/{}/contents/{}", owner, repo, path.trim_start_matches('/'));
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
struct OAuthTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubUser {
    pub login: String,
    pub id: i64,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}
